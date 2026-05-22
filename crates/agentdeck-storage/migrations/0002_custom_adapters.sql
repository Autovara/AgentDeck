-- 0002 — Custom adapters
--
-- User-defined "custom" adapters. Each row teaches AgentDeck how to detect
-- one kind of agent it does not ship native support for. Detection is the
-- only capability level allowed (Level 1, per adapter-feasibility.md §4.4);
-- status/usage/control are deliberately absent from this schema.
--
-- Conventions:
--
-- - `id` is a UUID v4 string supplied by the application (the storage layer
--   does not generate it).
-- - `label` is the user-visible name; unique so the dashboard can use it as
--   a stable identifier without surfacing the UUID.
-- - `agent_name` is what the matched session reports for its `agent_name`
--   column when the adapter framework later starts creating sessions. It is
--   intentionally a separate field from `label` so users can group multiple
--   "flavours" of the same agent (e.g. an `aider-prod` and an `aider-dev`
--   label both sharing `agent_name = "aider"`).
-- - `match_kind` is a CHECK-enforced enum: `name` matches the process name,
--   `cmdline` matches the joined command line, `cwd` matches the process's
--   working directory. Patterns are full Rust regexes; the storage layer
--   does not compile them.
-- - `cost_per_hour_cents` is informational only. Adapters above Level 1
--   report exact usage instead; this field exists so custom adapters can
--   contribute a coarse cost-per-hour estimate.

CREATE TABLE custom_adapters (
    id                   TEXT    PRIMARY KEY,
    label                TEXT    NOT NULL UNIQUE,
    enabled              INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    agent_name           TEXT    NOT NULL,
    color                TEXT,
    match_kind           TEXT    NOT NULL CHECK (
                             match_kind IN ('name', 'cmdline', 'cwd')
                         ),
    pattern              TEXT    NOT NULL,
    cost_per_hour_cents  INTEGER,
    notes                TEXT,
    created_at           TEXT    NOT NULL,
    updated_at           TEXT    NOT NULL
) STRICT;

CREATE INDEX idx_custom_adapters_enabled ON custom_adapters(enabled);
CREATE INDEX idx_custom_adapters_agent   ON custom_adapters(agent_name);
