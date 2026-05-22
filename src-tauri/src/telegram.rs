//! Tauri-facing wrapper around `agentdeck-telegram`.
//!
//! Owns the live bot handle plus the in-memory pairing state, and
//! exposes the eight commands the Settings page calls into:
//!
//! - [`snapshot`] / `get_telegram_status` — current state for the card
//! - [`set_token`] / [`clear_token`] — persist (and not log) the token
//! - [`enable`] / [`disable`] — toggle the user-visible feature flag
//! - [`generate_pairing_code`] / [`cancel_pairing`] — pairing flow
//! - [`revoke_user`] — drop a paired Telegram user
//!
//! Bot lifecycle is owned here: the bot task runs when
//! `(enabled && token_present)` and is rebuilt from scratch on every
//! state-changing command. That keeps the lifecycle simple and the
//! corner cases obvious (re-pair after disable, swap token, etc.).

use std::sync::{Arc, Mutex};

use agentdeck_attention::AttentionEngine;
use agentdeck_core::{Clock, SystemClock};
use agentdeck_storage::Storage;
use agentdeck_telegram::{
    allowlist, audit,
    bot::{self, BotContext, BotHandle},
    pairing::{self, PairingState, DEFAULT_EXPIRY},
    settings, AllowlistEntry, RateLimiter, TelegramError,
};
use chrono::{DateTime, Utc};
use serde::Serialize;

/// Dashboard payload for the Telegram pairing card.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TelegramStatusReport {
    /// `true` when storage opened at startup.
    pub ready: bool,
    /// Has the user opted Telegram on?
    pub enabled: bool,
    /// Whether a bot token is configured. The token itself is never
    /// returned to the dashboard.
    pub has_token: bool,
    /// Whether the bot task is currently running. Distinct from
    /// `enabled`: a fresh enable with no token leaves `enabled = true`
    /// but `running = false`.
    pub running: bool,
    /// Paired users. Sorted by `pairedAt` ascending so the list is
    /// stable across reloads.
    pub allowlist: Vec<AllowlistEntry>,
    /// Pending pairing code, if a generation is currently active and
    /// has not expired or been consumed.
    pub pending_pairing: Option<PendingPairingPayload>,
    /// UTC timestamp the report was assembled.
    pub captured_at: DateTime<Utc>,
    /// Error, if any of the storage reads failed.
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingPairingPayload {
    pub code: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingCodeResult {
    pub code: String,
    pub expires_at: DateTime<Utc>,
    pub status: TelegramStatusReport,
}

/// Tauri-managed Telegram state.
pub struct TelegramState {
    inner: Option<Inner>,
}

struct Inner {
    storage: Arc<Storage>,
    pairing: Arc<PairingState>,
    attention: Arc<AttentionEngine>,
    rate_limiter: Arc<RateLimiter>,
    bot: Mutex<Option<BotHandle>>,
}

impl TelegramState {
    pub fn new(storage: Option<Arc<Storage>>) -> Self {
        Self {
            inner: storage.map(|storage| {
                let clock: Arc<dyn Clock> = Arc::new(SystemClock);
                Inner {
                    attention: Arc::new(AttentionEngine::new(storage.clone(), clock)),
                    rate_limiter: Arc::new(RateLimiter::with_defaults()),
                    pairing: Arc::new(PairingState::new()),
                    storage,
                    bot: Mutex::new(None),
                }
            }),
        }
    }

    /// Best-effort startup hook: if the previous session had Telegram
    /// enabled with a valid token, start the bot. Failures degrade
    /// silently so a transient storage error never blocks app launch.
    pub async fn maybe_start_on_boot(&self) {
        let Some(inner) = self.inner.as_ref() else {
            return;
        };
        let enabled = settings::get_enabled(&inner.storage).unwrap_or(false);
        let token = settings::get_token(&inner.storage).unwrap_or(None);
        if let (true, Some(token)) = (enabled, token) {
            restart_bot(inner, Some(token)).await;
        }
    }

    /// Stop the bot task on app shutdown. Safe to call when no bot is
    /// running. Reserved for a future Tauri `CloseRequested` hook;
    /// until then the OS reclaims the polling task at process exit.
    #[allow(dead_code)]
    pub async fn shutdown(&self) {
        let Some(inner) = self.inner.as_ref() else {
            return;
        };
        let handle = take_handle(&inner.bot);
        if let Some(h) = handle {
            h.shutdown().await;
        }
    }
}

/// `get_telegram_status` command implementation.
pub fn snapshot(state: &TelegramState) -> TelegramStatusReport {
    let Some(inner) = state.inner.as_ref() else {
        return TelegramStatusReport::unavailable("storage unavailable");
    };
    build_snapshot(inner)
}

/// `set_telegram_token` command. Saves the token, restarts the bot
/// when applicable, never logs the token.
pub async fn set_token(state: &TelegramState, token: String) -> Result<TelegramStatusReport, String> {
    let inner = require_inner(state)?;
    let now = Utc::now();
    settings::set_token(&inner.storage, &token, now).map_err(stringify)?;
    if let Err(err) = audit::write_telegram_token_changed(&inner.storage, true, now) {
        tracing::warn!(error = %err, "telegram: audit log write failed for token set");
    }
    if settings::get_enabled(&inner.storage).unwrap_or(false) {
        restart_bot(inner, Some(token)).await;
    }
    Ok(build_snapshot(inner))
}

/// `clear_telegram_token` command. Stops the bot too (the token is the
/// only thing that lets the bot connect).
pub async fn clear_token(state: &TelegramState) -> Result<TelegramStatusReport, String> {
    let inner = require_inner(state)?;
    let now = Utc::now();
    settings::clear_token(&inner.storage, now).map_err(stringify)?;
    if let Err(err) = audit::write_telegram_token_changed(&inner.storage, false, now) {
        tracing::warn!(error = %err, "telegram: audit log write failed for token clear");
    }
    inner.pairing.clear();
    stop_bot(inner).await;
    Ok(build_snapshot(inner))
}

/// `enable_telegram` command. Persists the flag and starts the bot if
/// a token is configured.
pub async fn enable(state: &TelegramState) -> Result<TelegramStatusReport, String> {
    let inner = require_inner(state)?;
    let now = Utc::now();
    settings::set_enabled(&inner.storage, true, now).map_err(stringify)?;
    if let Err(err) = audit::write_telegram_enabled(&inner.storage, true, now) {
        tracing::warn!(error = %err, "telegram: audit log write failed for enable");
    }
    let token = settings::get_token(&inner.storage).map_err(stringify)?;
    if let Some(t) = token {
        restart_bot(inner, Some(t)).await;
    }
    Ok(build_snapshot(inner))
}

/// `disable_telegram` command. Persists the flag, stops the bot, and
/// drops any pending pairing.
pub async fn disable(state: &TelegramState) -> Result<TelegramStatusReport, String> {
    let inner = require_inner(state)?;
    let now = Utc::now();
    settings::set_enabled(&inner.storage, false, now).map_err(stringify)?;
    if let Err(err) = audit::write_telegram_enabled(&inner.storage, false, now) {
        tracing::warn!(error = %err, "telegram: audit log write failed for disable");
    }
    inner.pairing.clear();
    stop_bot(inner).await;
    Ok(build_snapshot(inner))
}

/// `generate_pairing_code` command. Replaces any pending code.
pub fn generate_pairing_code(state: &TelegramState) -> Result<PairingCodeResult, String> {
    let inner = require_inner(state)?;
    let now = Utc::now();
    let code = pairing::generate_pairing_code();
    let expires_at = now + DEFAULT_EXPIRY;
    inner.pairing.start(&code, expires_at);
    if let Err(err) = audit::write_pairing_code_generated(&inner.storage, now) {
        tracing::warn!(error = %err, "telegram: audit log write failed for code generation");
    }
    Ok(PairingCodeResult {
        code,
        expires_at,
        status: build_snapshot(inner),
    })
}

/// `cancel_pairing` command. Drops any pending code so the user can
/// start over without waiting for the expiry.
pub fn cancel_pairing(state: &TelegramState) -> Result<TelegramStatusReport, String> {
    let inner = require_inner(state)?;
    inner.pairing.clear();
    Ok(build_snapshot(inner))
}

/// `revoke_telegram_user` command. Removes a paired user from the
/// allowlist.
pub fn revoke_user(state: &TelegramState, user_id: i64) -> Result<TelegramStatusReport, String> {
    let inner = require_inner(state)?;
    let now = Utc::now();
    allowlist::remove(&inner.storage, user_id, now).map_err(stringify)?;
    if let Err(err) = audit::write_telegram_revoked(&inner.storage, user_id, now) {
        tracing::warn!(error = %err, "telegram: audit log write failed for revoke");
    }
    Ok(build_snapshot(inner))
}

fn build_snapshot(inner: &Inner) -> TelegramStatusReport {
    let now = Utc::now();
    let enabled = settings::get_enabled(&inner.storage).unwrap_or(false);
    let token = settings::get_token(&inner.storage).unwrap_or(None);
    let mut allowlist = allowlist::load(&inner.storage).unwrap_or_default();
    allowlist.sort_by_key(|e| e.paired_at);

    let pending_pairing = inner.pairing.snapshot(now).map(|p| PendingPairingPayload {
        code: p.code,
        expires_at: p.expires_at,
    });

    let running = bot_is_running(&inner.bot);

    TelegramStatusReport {
        ready: true,
        enabled,
        has_token: token.is_some(),
        running,
        allowlist,
        pending_pairing,
        captured_at: now,
        error: None,
    }
}

impl TelegramStatusReport {
    fn unavailable(message: impl Into<String>) -> Self {
        Self {
            ready: false,
            enabled: false,
            has_token: false,
            running: false,
            allowlist: Vec::new(),
            pending_pairing: None,
            captured_at: Utc::now(),
            error: Some(message.into()),
        }
    }
}

/// Always stop the current bot (if any) before starting a fresh one.
/// `restart_bot(inner, None)` is just a stop.
async fn restart_bot(inner: &Inner, token: Option<String>) {
    stop_bot(inner).await;
    let Some(token) = token else { return };
    let ctx = BotContext {
        storage: inner.storage.clone(),
        pairing: inner.pairing.clone(),
        attention: inner.attention.clone(),
        rate_limiter: inner.rate_limiter.clone(),
    };
    let handle = bot::start_bot(token, ctx);
    let mut guard = inner.bot.lock().expect("telegram bot mutex poisoned");
    debug_assert!(guard.is_none(), "restart_bot must run after stop_bot");
    *guard = Some(handle);
}

async fn stop_bot(inner: &Inner) {
    let handle = take_handle(&inner.bot);
    if let Some(h) = handle {
        h.shutdown().await;
    }
}

fn take_handle(slot: &Mutex<Option<BotHandle>>) -> Option<BotHandle> {
    slot.lock().expect("telegram bot mutex poisoned").take()
}

fn bot_is_running(slot: &Mutex<Option<BotHandle>>) -> bool {
    slot.lock()
        .ok()
        .map(|g| g.is_some())
        .unwrap_or(false)
}

fn require_inner(state: &TelegramState) -> Result<&Inner, String> {
    state
        .inner
        .as_ref()
        .ok_or_else(|| "storage unavailable".to_string())
}

fn stringify(err: TelegramError) -> String {
    err.to_string()
}
