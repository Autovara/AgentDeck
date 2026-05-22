//! Telegram bot lifecycle: start, message handler, shutdown.
//!
//! The bot uses teloxide's long-polling [`Dispatcher`]. There is a
//! single message handler in this module; the per-command formatting
//! lives in [`crate::commands`] so it can be unit-tested without a live
//! bot.
//!
//! Message flow:
//!
//! 1. Pairing first — `PAIR <code>` is handled outside the allowlist
//!    and outside the rate limiter. A user who is in the middle of
//!    pairing must not be locked out by their own retries.
//! 2. Identify the sender via `msg.from.id`. Messages with no
//!    identifiable user (channel posts, anonymous forwards) are
//!    silently ignored.
//! 3. Allowlist gate. Non-paired users get a single reply explaining
//!    the pairing flow.
//! 4. Rate limit. Paired users with a depleted bucket get a
//!    "try again in Ns" reply.
//! 5. Parse and dispatch slash commands. Anything else (free-form
//!    text from a paired user) is ignored; the alpha bot is
//!    command-only.

use std::sync::Arc;

use agentdeck_attention::AttentionEngine;
use agentdeck_session::{list_active, Session};
use agentdeck_storage::Storage;
use chrono::{Duration, Utc};
use teloxide::dispatching::ShutdownToken;
use teloxide::prelude::*;
use tokio::task::JoinHandle;

use crate::audit::StopAuditResult;
use crate::commands::{
    format_agents, format_attention, format_help, format_mute_all_failed,
    format_mute_no_attention, format_mute_out_of_range, format_mute_success, format_mute_usage,
    format_rate_limited, format_session_ambiguous, format_session_detail, format_session_not_found,
    format_session_usage, format_status, format_stop_already_completed, format_stop_expired,
    format_stop_failed, format_stop_mismatch, format_stop_no_pending, format_stop_prompt,
    format_stop_session_not_found, format_stop_success, format_stop_unsupported,
    format_stop_usage, format_unknown_command, lookup_session, parse_command, short_session_id,
    BotCommand, SessionLookup, DEFAULT_MUTE_HOURS, MAX_MUTE_HOURS,
};
use crate::pairing::{PairingState, TryConsume};
use crate::rate_limit::{RateLimitOutcome, RateLimiter};
use crate::stop::{
    PendingStop, StopConfirmationState, StopDispatcher, StopOutcome, TryConsumeStop,
    STOP_CONFIRMATION_WINDOW,
};
use crate::{allowlist, audit, remote_commands};
use uuid::Uuid;

/// Shared state every bot handler gets. Wrapped in an `Arc` so the
/// teloxide dispatcher can clone it into each per-message handler
/// invocation.
#[derive(Clone)]
pub struct BotContext {
    pub storage: Arc<Storage>,
    pub pairing: Arc<PairingState>,
    pub attention: Arc<AttentionEngine>,
    pub rate_limiter: Arc<RateLimiter>,
    pub stop_confirmations: Arc<StopConfirmationState>,
    pub stop_dispatcher: Arc<dyn StopDispatcher>,
}

/// Handle returned by [`start_bot`]. Calling [`BotHandle::shutdown`]
/// requests teloxide to stop after the current poll, then awaits the
/// task. Dropping the handle without shutting down aborts the task
/// (teloxide can be restarted from scratch, so a hard abort is
/// acceptable on app exit).
pub struct BotHandle {
    shutdown: ShutdownToken,
    handle: JoinHandle<()>,
}

impl BotHandle {
    /// Request a clean shutdown of the bot. Best-effort; if teloxide
    /// is already shutting down or the task has panicked the error is
    /// swallowed at `tracing::debug!`.
    pub async fn shutdown(self) {
        match self.shutdown.shutdown() {
            Ok(fut) => fut.await,
            Err(err) => {
                tracing::debug!(error = %err, "telegram bot shutdown token unavailable; abort instead");
                self.handle.abort();
            }
        }
        if let Err(err) = self.handle.await {
            if !err.is_cancelled() {
                tracing::warn!(error = %err, "telegram bot task ended abnormally");
            }
        }
    }
}

/// Start the bot with `token` and the shared `ctx`. Returns
/// immediately; the bot runs on a tokio task.
pub fn start_bot(token: String, ctx: BotContext) -> BotHandle {
    let bot = Bot::new(token);

    // Note: we deliberately do not call `enable_ctrlc_handler()` — the
    // Tauri shell already owns process signals and a second handler
    // would race with it on shutdown.
    let handler = Update::filter_message().endpoint(handle_message);
    let mut dispatcher = Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![ctx])
        .build();
    let shutdown = dispatcher.shutdown_token();

    let handle = tokio::spawn(async move {
        tracing::info!("telegram bot starting");
        dispatcher.dispatch().await;
        tracing::info!("telegram bot stopped");
    });

    BotHandle { shutdown, handle }
}

/// One Telegram message → at most one reply. Errors are logged and
/// swallowed so a single malformed update never tears down the bot.
async fn handle_message(bot: Bot, msg: Message, ctx: BotContext) -> ResponseResult<()> {
    let Some(text) = msg.text().map(str::trim) else {
        return Ok(());
    };
    let chat_id = msg.chat.id;
    let user_id_opt = msg.from.as_ref().map(|u| u.id.0 as i64);

    // Pairing always comes first and is not rate-limited.
    if let Some(rest) = text.strip_prefix("PAIR ").or_else(|| text.strip_prefix("pair ")) {
        let code = rest.trim().to_ascii_uppercase();
        return reply_pairing(bot, chat_id, user_id_opt, &msg, &code, ctx).await;
    }

    let Some(user_id) = user_id_opt else {
        return Ok(());
    };

    let allowed = allowlist::contains(&ctx.storage, user_id).unwrap_or(false);
    if !allowed {
        bot.send_message(
            chat_id,
            "AgentDeck is not paired with you. \
             Open the AgentDeck app and generate a pairing code, then send `PAIR <code>` here.",
        )
        .await?;
        return Ok(());
    }

    // Apply rate limit before doing any storage reads.
    if let RateLimitOutcome::Throttled { retry_after_secs } =
        ctx.rate_limiter.check(user_id, Utc::now())
    {
        bot.send_message(
            chat_id,
            format_rate_limited(std::time::Duration::from_secs(retry_after_secs as u64)),
        )
        .await?;
        return Ok(());
    }

    // STOP <code> confirmation arrives as free-form text (paralleling
    // PAIR). Match it before the slash-command parser since the
    // parser would otherwise reject it.
    if let Some(rest) = text.strip_prefix("STOP ").or_else(|| text.strip_prefix("stop ")) {
        return reply_stop_confirmation(&bot, chat_id, user_id, rest.trim(), &ctx).await;
    }

    // Parse the slash command. Free-form text from paired users is
    // ignored; the alpha bot is command-only.
    let Some(command) = parse_command(text) else {
        return Ok(());
    };
    dispatch_command(&bot, chat_id, user_id, &ctx, command).await
}

async fn dispatch_command(
    bot: &Bot,
    chat_id: ChatId,
    user_id: i64,
    ctx: &BotContext,
    command: BotCommand,
) -> ResponseResult<()> {
    let reply = match command {
        BotCommand::Help => format_help(),
        BotCommand::Status => match build_status_reply(ctx).await {
            Ok(s) => s,
            Err(s) => s,
        },
        BotCommand::Agents => match build_agents_reply(ctx).await {
            Ok(s) => s,
            Err(s) => s,
        },
        BotCommand::Attention => match build_attention_reply(ctx).await {
            Ok(s) => s,
            Err(s) => s,
        },
        BotCommand::Session(arg) => match build_session_reply(ctx, &arg).await {
            Ok(s) => s,
            Err(s) => s,
        },
        BotCommand::SessionUsage => format_session_usage(),
        BotCommand::Mute { id, hours } => match build_mute_reply(ctx, &id, hours).await {
            Ok(s) => s,
            Err(s) => s,
        },
        BotCommand::MuteUsage => format_mute_usage(),
        BotCommand::Stop(arg) => match build_stop_prompt_reply(ctx, user_id, &arg).await {
            Ok(s) => s,
            Err(s) => s,
        },
        BotCommand::StopUsage => format_stop_usage(),
        BotCommand::Unknown(name) => format_unknown_command(&name),
    };
    bot.send_message(chat_id, reply).await?;
    Ok(())
}

async fn build_status_reply(ctx: &BotContext) -> Result<String, String> {
    let sessions = load_active_sessions(ctx)?;
    let attention = ctx
        .attention
        .list_open()
        .map_err(|e| format!("Could not read attention items: {e}"))?;
    Ok(format_status(&sessions, &attention))
}

async fn build_agents_reply(ctx: &BotContext) -> Result<String, String> {
    let sessions = load_active_sessions(ctx)?;
    Ok(format_agents(&sessions, Utc::now()))
}

async fn build_attention_reply(ctx: &BotContext) -> Result<String, String> {
    let attention = ctx
        .attention
        .list_open()
        .map_err(|e| format!("Could not read attention items: {e}"))?;
    Ok(format_attention(&attention, Utc::now()))
}

async fn build_session_reply(ctx: &BotContext, query: &str) -> Result<String, String> {
    let sessions = load_active_sessions(ctx)?;
    match lookup_session(query, &sessions) {
        SessionLookup::NotFound => Ok(format_session_not_found(query)),
        SessionLookup::Ambiguous(candidates) => Ok(format_session_ambiguous(query, &candidates)),
        SessionLookup::Found(session) => {
            let attention = ctx
                .attention
                .list_open()
                .map_err(|e| format!("Could not read attention items: {e}"))?;
            let for_session: Vec<_> = attention
                .iter()
                .filter(|e| e.item.session_id == session.id)
                .map(|e| e.item.clone())
                .collect();
            Ok(format_session_detail(session, &for_session, Utc::now()))
        }
    }
}

fn load_active_sessions(ctx: &BotContext) -> Result<Vec<Session>, String> {
    list_active(&ctx.storage).map_err(|e| format!("Could not read sessions: {e}"))
}

async fn build_mute_reply(
    ctx: &BotContext,
    query: &str,
    hours: Option<u32>,
) -> Result<String, String> {
    let hours = hours.unwrap_or(DEFAULT_MUTE_HOURS);
    if !(1..=MAX_MUTE_HOURS).contains(&hours) {
        return Ok(format_mute_out_of_range(hours));
    }

    let sessions = load_active_sessions(ctx)?;
    let session = match lookup_session(query, &sessions) {
        SessionLookup::NotFound => return Ok(format_session_not_found(query)),
        SessionLookup::Ambiguous(candidates) => {
            return Ok(format_session_ambiguous(query, &candidates));
        }
        SessionLookup::Found(s) => s,
    };

    let open = ctx
        .attention
        .list_open()
        .map_err(|e| format!("Could not read attention items: {e}"))?;
    let for_session: Vec<_> = open
        .iter()
        .filter(|e| e.item.session_id == session.id)
        .collect();
    if for_session.is_empty() {
        return Ok(format_mute_no_attention(session));
    }

    let now = Utc::now();
    let until = now + Duration::hours(hours as i64);
    let mut muted = 0_usize;
    for entry in &for_session {
        match ctx.attention.set_mute(entry.item.id, Some(until)) {
            Ok(_) => {
                muted += 1;
                if let Err(err) = audit::write_telegram_attention_muted(
                    &ctx.storage,
                    entry.item.id,
                    session.id,
                    hours,
                    until,
                    now,
                ) {
                    tracing::warn!(
                        error = %err,
                        attention_id = %entry.item.id,
                        "telegram: audit log write failed for /mute",
                    );
                }
            }
            Err(err) => {
                tracing::warn!(
                    error = %err,
                    attention_id = %entry.item.id,
                    "telegram: set_mute failed",
                );
            }
        }
    }

    if muted == 0 {
        return Ok(format_mute_all_failed());
    }
    Ok(format_mute_success(session, muted, hours, until))
}

/// Handle `/stop <id>`: stage a pending confirmation, persist a
/// `remote_commands` row, write the audit row, and reply with the
/// confirmation prompt.
async fn build_stop_prompt_reply(
    ctx: &BotContext,
    user_id: i64,
    query: &str,
) -> Result<String, String> {
    let sessions = load_active_sessions(ctx)?;
    let session = match lookup_session(query, &sessions) {
        SessionLookup::NotFound => return Ok(format_session_not_found(query)),
        SessionLookup::Ambiguous(candidates) => {
            return Ok(format_session_ambiguous(query, &candidates));
        }
        SessionLookup::Found(s) => s,
    };

    let now = Utc::now();
    let command_id = Uuid::new_v4();
    let short_id = short_session_id(session.id);

    // Persist the pending command row first so the audit trail is
    // never lighter than the in-memory state.
    if let Err(err) = remote_commands::insert_pending_stop(
        &ctx.storage,
        command_id,
        user_id,
        session.id,
        now,
    ) {
        tracing::warn!(error = %err, "telegram: failed to insert pending stop row");
        return Ok("Could not stage the stop request. Try again.".to_string());
    }
    if let Err(err) =
        audit::write_stop_requested(&ctx.storage, command_id, session.id, user_id, now)
    {
        tracing::warn!(error = %err, "telegram: audit log write failed for /stop");
    }

    ctx.stop_confirmations.start(
        user_id,
        PendingStop {
            session_id: session.id,
            short_id: short_id.clone(),
            command_id,
            expires_at: now + STOP_CONFIRMATION_WINDOW,
        },
    );

    Ok(format_stop_prompt(session, &short_id))
}

/// Handle the free-form `STOP <code>` confirmation: consume the pending
/// slot, dispatch via the platform stop, update `remote_commands`,
/// write the audit row, and reply with the outcome.
async fn reply_stop_confirmation(
    bot: &Bot,
    chat_id: ChatId,
    user_id: i64,
    code: &str,
    ctx: &BotContext,
) -> ResponseResult<()> {
    let now = Utc::now();
    let pending = match ctx.stop_confirmations.try_consume(user_id, code, now) {
        TryConsumeStop::NoPending => {
            bot.send_message(chat_id, format_stop_no_pending(code)).await?;
            return Ok(());
        }
        TryConsumeStop::Expired => {
            bot.send_message(chat_id, format_stop_expired()).await?;
            return Ok(());
        }
        TryConsumeStop::Mismatch => {
            bot.send_message(chat_id, format_stop_mismatch()).await?;
            return Ok(());
        }
        TryConsumeStop::Matched(p) => p,
    };

    // Hand off to the dispatcher. The trait method is synchronous; the
    // platform implementations all complete in milliseconds.
    let outcome = ctx.stop_dispatcher.stop(pending.session_id);
    let (reply, audit_result, mechanism, error_text, rc_success, rc_error) =
        classify_outcome(&pending.short_id, &outcome);

    // Update remote_commands lifecycle.
    if let Err(err) = remote_commands::mark_executed(
        &ctx.storage,
        pending.command_id,
        rc_success,
        rc_error.as_deref(),
        now,
    ) {
        tracing::warn!(
            error = %err,
            command_id = %pending.command_id,
            "telegram: failed to update remote_commands after /stop",
        );
    }

    if let Err(err) = audit::write_stop_executed(
        &ctx.storage,
        pending.command_id,
        pending.session_id,
        user_id,
        audit_result,
        mechanism.as_deref(),
        error_text.as_deref(),
        now,
    ) {
        tracing::warn!(
            error = %err,
            command_id = %pending.command_id,
            "telegram: audit log write failed for STOP confirmation",
        );
    }

    bot.send_message(chat_id, reply).await?;
    Ok(())
}

/// Map a [`StopOutcome`] to (reply, audit_result, mechanism, error_text,
/// remote_commands.success, remote_commands.error).
fn classify_outcome(
    short_id: &str,
    outcome: &StopOutcome,
) -> (
    String,
    StopAuditResult,
    Option<String>,
    Option<String>,
    bool,
    Option<String>,
) {
    match outcome {
        StopOutcome::Success { mechanism } => (
            format_stop_success(short_id, mechanism),
            StopAuditResult::Success,
            Some(mechanism.clone()),
            None,
            true,
            None,
        ),
        StopOutcome::SessionNotFound => (
            format_stop_session_not_found(short_id),
            StopAuditResult::AuditOnly,
            None,
            None,
            false,
            Some("session_not_found".to_string()),
        ),
        StopOutcome::AlreadyCompleted => (
            format_stop_already_completed(short_id),
            StopAuditResult::AuditOnly,
            None,
            None,
            false,
            Some("already_completed".to_string()),
        ),
        StopOutcome::Unsupported { reason } => (
            format_stop_unsupported(short_id, reason),
            StopAuditResult::AuditOnly,
            None,
            Some(reason.clone()),
            false,
            Some(format!("unsupported: {reason}")),
        ),
        StopOutcome::Failed { reason } => (
            format_stop_failed(short_id, reason),
            StopAuditResult::Failed,
            None,
            Some(reason.clone()),
            false,
            Some(reason.clone()),
        ),
    }
}

async fn reply_pairing(
    bot: Bot,
    chat_id: ChatId,
    user_id_opt: Option<i64>,
    msg: &Message,
    code: &str,
    ctx: BotContext,
) -> ResponseResult<()> {
    let Some(user_id) = user_id_opt else {
        bot.send_message(
            chat_id,
            "Could not identify your Telegram user. Pairing aborted.",
        )
        .await?;
        return Ok(());
    };

    let now = Utc::now();
    match ctx.pairing.try_consume(code, now) {
        TryConsume::NoPending => {
            bot.send_message(
                chat_id,
                "No pairing in progress. Generate a code from the AgentDeck app first.",
            )
            .await?;
        }
        TryConsume::Expired => {
            bot.send_message(
                chat_id,
                "That pairing code has expired. Generate a new one in AgentDeck.",
            )
            .await?;
        }
        TryConsume::Mismatch => {
            bot.send_message(chat_id, "Invalid pairing code.").await?;
        }
        TryConsume::Matched => {
            let username = msg.from.as_ref().and_then(|u| u.username.clone());
            if let Err(err) = allowlist::add(
                &ctx.storage,
                crate::AllowlistEntry {
                    user_id,
                    username: username.clone(),
                    paired_at: now,
                },
                now,
            ) {
                tracing::warn!(error = %err, "telegram: failed to persist allowlist after pair");
                bot.send_message(
                    chat_id,
                    "Pairing succeeded but AgentDeck failed to record it. Please try again.",
                )
                .await?;
                return Ok(());
            }
            if let Err(err) = audit::write_telegram_paired(&ctx.storage, user_id, username.as_deref(), now) {
                tracing::warn!(error = %err, "telegram: audit log write failed for pairing");
            }
            let label = username
                .as_deref()
                .map(|u| format!("@{u}"))
                .unwrap_or_else(|| format!("id {user_id}"));
            bot.send_message(
                chat_id,
                format!(
                    "✓ Paired as {label}. Send /help to see available commands."
                ),
            )
            .await?;
        }
    }
    Ok(())
}
