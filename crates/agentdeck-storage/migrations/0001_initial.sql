-- AgentDeck initial schema.
--
-- This file is the *only* place the alpha database shape is allowed to be
-- defined. Future migrations must be added as additional numbered files and
-- registered in `src/schema.rs`; never edit a migration once it has shipped.
--
-- Conventions:
--
-- - All timestamps are ISO-8601 UTC strings (TEXT). chrono's serialisation
--   format. SQLite can sort them lexicographically.
-- - All ids that come from outside SQLite (sessions, attention items, project
--   tags, remote commands) are UUID v4 strings.
-- - All ids that are pure SQLite bookkeeping (session_events, usage_records,
--   audit_log) use INTEGER PRIMARY KEY autoincrement.
-- - JSON-ish blobs are stored as TEXT; the storage layer does not enforce
--   their shape, the monitor core does.
--
-- Retention defaults from alpha-build-plan §14 are documented next to each
-- table; the retention pruner that enforces them lands in a later step.

PRAGMA foreign_keys = ON;

-- =========================================================================
-- Sessions: one row per detected agent session.
-- Retention: 90 days after last_seen_time (user-configurable in settings).
-- =========================================================================
CREATE TABLE sessions (
    id                  TEXT    PRIMARY KEY,
    agent_name          TEXT    NOT NULL,
    adapter_name        TEXT    NOT NULL,
    adapter_level       INTEGER NOT NULL CHECK (adapter_level BETWEEN 1 AND 4),
    pid                 INTEGER,
    command             TEXT    NOT NULL,
    cwd                 TEXT,
    repo_path           TEXT,
    project_tag         TEXT,
    status              TEXT    NOT NULL CHECK (status IN (
                            'running','idle','waiting_for_input','rate_limited',
                            'stalled','errored','completed','unknown'
                        )),
    status_confidence   TEXT    NOT NULL DEFAULT 'unknown' CHECK (
                            status_confidence IN ('high','medium','low','unknown')
                        ),
    attention_reason    TEXT,
    start_time          TEXT    NOT NULL,
    last_seen_time      TEXT    NOT NULL,
    last_activity_time  TEXT,
    estimated_cost      REAL,
    cost_kind           TEXT    NOT NULL DEFAULT 'unknown' CHECK (
                            cost_kind IN ('exact','estimated','unknown')
                        ),
    created_at          TEXT    NOT NULL,
    updated_at          TEXT    NOT NULL
) STRICT;

CREATE INDEX idx_sessions_status         ON sessions(status);
CREATE INDEX idx_sessions_updated_at     ON sessions(updated_at);
CREATE INDEX idx_sessions_adapter        ON sessions(adapter_name);
CREATE INDEX idx_sessions_project_tag    ON sessions(project_tag);

-- =========================================================================
-- Session events: append-only audit of state transitions per session.
-- Retention: 90 days. The retention pruner deletes rows older than the cutoff.
-- =========================================================================
CREATE TABLE session_events (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id  TEXT    NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    event_kind  TEXT    NOT NULL,
    payload     TEXT,                     -- optional JSON blob
    created_at  TEXT    NOT NULL
) STRICT;

CREATE INDEX idx_session_events_session_time
    ON session_events(session_id, created_at);
CREATE INDEX idx_session_events_kind
    ON session_events(event_kind, created_at);

-- =========================================================================
-- Attention items: things needing a human. Resolved items linger so the
-- dashboard can show recent attention history.
-- Retention: 90 days after resolved_at.
-- =========================================================================
CREATE TABLE attention_items (
    id                  TEXT    PRIMARY KEY,
    session_id          TEXT    NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    reason              TEXT    NOT NULL,
    severity            TEXT    NOT NULL CHECK (severity IN ('info','warn','urgent')),
    message             TEXT    NOT NULL,
    source              TEXT    NOT NULL,
    confidence          TEXT    NOT NULL CHECK (
                            confidence IN ('high','medium','low','unknown')
                        ),
    recommended_actions TEXT,               -- JSON array of action descriptors
    created_at          TEXT    NOT NULL,
    resolved_at         TEXT,
    muted_until         TEXT
) STRICT;

CREATE INDEX idx_attention_session         ON attention_items(session_id);
CREATE INDEX idx_attention_open
    ON attention_items(resolved_at)
    WHERE resolved_at IS NULL;
CREATE INDEX idx_attention_severity_time
    ON attention_items(severity, created_at);

-- =========================================================================
-- Adapter diagnostics: only the most recent scan per adapter is kept.
-- The PK is the adapter_name itself; INSERT OR REPLACE overwrites prior rows.
-- =========================================================================
CREATE TABLE adapter_diagnostics (
    adapter_name        TEXT    PRIMARY KEY,
    enabled             INTEGER NOT NULL CHECK (enabled IN (0,1)),
    capability_level    INTEGER NOT NULL CHECK (capability_level BETWEEN 1 AND 4),
    last_scan_time      TEXT    NOT NULL,
    detected_count      INTEGER NOT NULL DEFAULT 0,
    data_sources_used   TEXT,               -- JSON array
    missing_permissions TEXT,               -- JSON array
    failure_reasons     TEXT,               -- JSON array
    confidence          TEXT    NOT NULL CHECK (
                            confidence IN ('high','medium','low','unknown')
                        ),
    known_limitations   TEXT,               -- JSON array
    updated_at          TEXT    NOT NULL
) STRICT;

-- =========================================================================
-- Usage records: per-snapshot usage and cost data attributed to a session.
-- Retention: 13 months so monthly summaries remain across a fiscal year.
-- =========================================================================
CREATE TABLE usage_records (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id          TEXT    REFERENCES sessions(id) ON DELETE SET NULL,
    adapter_name        TEXT    NOT NULL,
    model               TEXT,
    input_tokens        INTEGER,
    output_tokens       INTEGER,
    cost_usd            REAL,
    cost_kind           TEXT    NOT NULL CHECK (
                            cost_kind IN ('exact','estimated','unknown')
                        ),
    pricing_source      TEXT,
    captured_at         TEXT    NOT NULL
) STRICT;

CREATE INDEX idx_usage_session_time ON usage_records(session_id, captured_at);
CREATE INDEX idx_usage_captured_at  ON usage_records(captured_at);
CREATE INDEX idx_usage_adapter_time ON usage_records(adapter_name, captured_at);

-- =========================================================================
-- Project tags: user-defined groupings for sessions (project/client labels).
-- =========================================================================
CREATE TABLE project_tags (
    id          TEXT    PRIMARY KEY,
    name        TEXT    NOT NULL UNIQUE,
    color       TEXT,
    notes       TEXT,
    created_at  TEXT    NOT NULL,
    updated_at  TEXT    NOT NULL
) STRICT;

-- =========================================================================
-- Remote commands: lifecycle of every Telegram (or other provider) command,
-- whether or not it was confirmed and executed. Companion to audit_log; this
-- table is the source of truth for "what was asked", audit_log is the source
-- of truth for "what was decided".
-- =========================================================================
CREATE TABLE remote_commands (
    id                      TEXT    PRIMARY KEY,
    provider                TEXT    NOT NULL,
    external_user_id        TEXT,
    command                 TEXT    NOT NULL,
    session_id              TEXT    REFERENCES sessions(id) ON DELETE SET NULL,
    requested_action        TEXT,
    status                  TEXT    NOT NULL CHECK (status IN (
                                'pending','confirmed','executed',
                                'denied','expired','failed'
                            )),
    requires_confirmation   INTEGER NOT NULL CHECK (requires_confirmation IN (0,1)),
    created_at              TEXT    NOT NULL,
    confirmed_at            TEXT,
    executed_at             TEXT,
    error                   TEXT
) STRICT;

CREATE INDEX idx_remote_commands_provider_time
    ON remote_commands(provider, created_at);
CREATE INDEX idx_remote_commands_status
    ON remote_commands(status, created_at);
CREATE INDEX idx_remote_commands_session
    ON remote_commands(session_id);

-- =========================================================================
-- Audit log: append-only record of every sensitive decision. Indefinite
-- retention by default; the user is the only entity allowed to clear it.
-- =========================================================================
CREATE TABLE audit_log (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    actor       TEXT    NOT NULL,
    source      TEXT    NOT NULL CHECK (
                    source IN ('tray','dashboard','telegram','cli','system','scheduler')
                ),
    action      TEXT    NOT NULL,
    target_type TEXT,
    target_id   TEXT,
    result      TEXT    NOT NULL CHECK (result IN (
                    'success','denied','failed','audit_only'
                )),
    metadata    TEXT,                       -- JSON
    created_at  TEXT    NOT NULL
) STRICT;

CREATE INDEX idx_audit_created_at   ON audit_log(created_at);
CREATE INDEX idx_audit_actor_time   ON audit_log(actor, created_at);
CREATE INDEX idx_audit_action_time  ON audit_log(action, created_at);

-- =========================================================================
-- Settings: typed key/value store. value is JSON; the storage layer treats
-- it as opaque text and leaves parsing to the monitor core.
-- =========================================================================
CREATE TABLE settings (
    key         TEXT    PRIMARY KEY,
    value       TEXT    NOT NULL,
    updated_at  TEXT    NOT NULL
) STRICT;

-- Seed the schema marker. Migration version is owned by PRAGMA user_version
-- (rusqlite_migration manages it). This row is informational only and is
-- safe to read from logs.
INSERT INTO settings (key, value, updated_at)
VALUES (
    'schema.bootstrap',
    json_object(
        'version', 1,
        'created_by', 'agentdeck-storage 0001_initial.sql'
    ),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
);
