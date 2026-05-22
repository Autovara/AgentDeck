//! Telegram bot lifecycle: start, message handler, shutdown.
//!
//! The bot uses teloxide's long-polling REPL. The single handler in
//! this module only services pairing in step 17; steps 18-20 will
//! grow it into a command dispatcher without changing the lifecycle
//! shape (start / shutdown remain the same).

use std::sync::Arc;

use agentdeck_storage::Storage;
use chrono::Utc;
use teloxide::dispatching::ShutdownToken;
use teloxide::prelude::*;
use tokio::task::JoinHandle;

use crate::pairing::{PairingState, TryConsume};
use crate::{allowlist, audit};

/// Shared state every bot handler gets. Wrapped in an `Arc` so the
/// teloxide dispatcher can clone it into each per-message handler
/// invocation.
#[derive(Clone)]
pub struct BotContext {
    pub storage: Arc<Storage>,
    pub pairing: Arc<PairingState>,
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

    if let Some(rest) = text.strip_prefix("PAIR ").or_else(|| text.strip_prefix("pair ")) {
        let code = rest.trim().to_ascii_uppercase();
        return reply_pairing(bot, chat_id, user_id_opt, &msg, &code, ctx).await;
    }

    // Not a pairing attempt. Gate on the allowlist; if the sender is
    // approved, point them at the commands not-yet-implemented stub.
    // If not, refuse without revealing whether the bot is configured.
    let user_id = match user_id_opt {
        Some(id) => id,
        None => return Ok(()),
    };

    let allowed = allowlist::contains(&ctx.storage, user_id).unwrap_or(false);
    if allowed {
        bot.send_message(
            chat_id,
            "Commands are not implemented yet in the AgentDeck alpha. \
             Watch for /help once they land.",
        )
        .await?;
    } else {
        bot.send_message(
            chat_id,
            "AgentDeck is not paired with you. \
             Open the AgentDeck app and generate a pairing code, then send `PAIR <code>` here.",
        )
        .await?;
    }
    Ok(())
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
                    "✓ Paired as {label}. AgentDeck commands will be available in a later alpha build."
                ),
            )
            .await?;
        }
    }
    Ok(())
}
