import { useMemo, useState } from "react";
import type { JSX } from "react";
import { AttentionList } from "../components/AttentionList";
import type {
  AttentionReason,
  AttentionReport,
  AttentionSeverity,
  OpenAttentionEntry,
} from "../lib/tauri";

type State =
  | { kind: "loading" }
  | { kind: "ready"; report: AttentionReport }
  | { kind: "error"; message: string };

type Props = {
  state: State;
  busy: boolean;
  onMute: (id: string, hours: number) => void;
  onResolve: (id: string) => void;
};

type SeverityFilter = "all" | AttentionSeverity;
type ReasonFilter = "all" | AttentionReason;

const SEVERITY_FILTERS: { id: SeverityFilter; label: string }[] = [
  { id: "all", label: "All" },
  { id: "urgent", label: "Urgent" },
  { id: "warn", label: "Warn" },
  { id: "info", label: "Info" },
];

const REASON_LABELS: Record<AttentionReason, string> = {
  waiting_for_input: "waiting for input",
  approval_required: "approval required",
  rate_limit: "rate limit",
  authentication_required: "auth required",
  context_limit: "context limit",
  stalled_session: "stalled",
  command_failed: "command failed",
  process_crashed: "process crashed",
  budget_threshold: "budget threshold",
};

export function AttentionPage({
  state,
  busy,
  onMute,
  onResolve,
}: Props): JSX.Element {
  const [severityFilter, setSeverityFilter] =
    useState<SeverityFilter>("all");
  const [reasonFilter, setReasonFilter] = useState<ReasonFilter>("all");
  const [showMuted, setShowMuted] = useState(false);

  const entries: OpenAttentionEntry[] =
    state.kind === "ready" ? state.report.items : [];

  const reasonOptions: ReasonFilter[] = useMemo(() => {
    const seen = new Set<AttentionReason>();
    for (const e of entries) seen.add(e.item.reason);
    const sorted = Array.from(seen).sort();
    return ["all", ...sorted];
  }, [entries]);

  const now = Date.now();
  const filtered = useMemo(() => {
    return entries.filter((entry) => {
      if (severityFilter !== "all" && entry.item.severity !== severityFilter) {
        return false;
      }
      if (reasonFilter !== "all" && entry.item.reason !== reasonFilter) {
        return false;
      }
      const muted =
        entry.item.mutedUntil != null &&
        new Date(entry.item.mutedUntil).getTime() > now;
      if (muted && !showMuted) return false;
      return true;
    });
  }, [entries, severityFilter, reasonFilter, showMuted, now]);

  return (
    <section className="page page--attention">
      <header className="page__header">
        <div>
          <h1 className="page__title">Attention</h1>
          <p className="page__subtitle">
            Every open attention item the monitor wants you to look at.
          </p>
        </div>
      </header>

      {state.kind === "loading" && (
        <div className="card">
          <p className="card__body">Loading attention items…</p>
        </div>
      )}

      {state.kind === "error" && (
        <div className="card">
          <div className="card__body card__body--error">
            <p>
              <strong>Attention engine unreachable.</strong>
            </p>
            <pre className="card__diagnostic">{state.message}</pre>
          </div>
        </div>
      )}

      {state.kind === "ready" && (
        <div className="page__body">
          {state.report.error != null && (
            <div className="card">
              <div className="card__body card__body--error">
                <pre className="card__diagnostic">{state.report.error}</pre>
              </div>
            </div>
          )}

          <section className="card">
            <div className="card__body">
              <div className="filters">
                <FilterGroup label="Severity">
                  {SEVERITY_FILTERS.map((f) => (
                    <FilterChip
                      key={f.id}
                      active={severityFilter === f.id}
                      onClick={() => setSeverityFilter(f.id)}
                    >
                      {f.label}
                    </FilterChip>
                  ))}
                </FilterGroup>

                <FilterGroup label="Reason">
                  <select
                    className="filters__select"
                    value={reasonFilter}
                    onChange={(e) =>
                      setReasonFilter(e.target.value as ReasonFilter)
                    }
                  >
                    {reasonOptions.map((r) => (
                      <option key={r} value={r}>
                        {r === "all" ? "All" : REASON_LABELS[r] ?? r}
                      </option>
                    ))}
                  </select>
                </FilterGroup>

                <FilterGroup label="Muted">
                  <label className="filters__checkbox">
                    <input
                      type="checkbox"
                      checked={showMuted}
                      onChange={(e) => setShowMuted(e.target.checked)}
                    />
                    <span>Show muted</span>
                  </label>
                </FilterGroup>

                <div className="filters__count">
                  {filtered.length} of {entries.length} shown
                </div>
              </div>

              <AttentionList
                entries={filtered}
                busy={busy}
                onMute={onMute}
                onResolve={onResolve}
                columns={{
                  severity: true,
                  reason: true,
                  session: true,
                  repo: true,
                  confidence: true,
                  message: true,
                  age: true,
                  actions: true,
                }}
                emptyMessage={
                  entries.length === 0 ? (
                    <>
                      Nothing is asking for attention right now. Items appear
                      here when a session enters <code>rate_limited</code>,{" "}
                      <code>errored</code>, <code>stalled</code>, or (for
                      Level&nbsp;2+ adapters) <code>waiting_for_input</code>.
                    </>
                  ) : (
                    <>No items match the current filters.</>
                  )
                }
              />
            </div>
          </section>
        </div>
      )}
    </section>
  );
}

function FilterGroup({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}): JSX.Element {
  return (
    <div className="filters__group">
      <span className="filters__label">{label}</span>
      <div className="filters__row">{children}</div>
    </div>
  );
}

function FilterChip({
  active,
  onClick,
  children,
}: {
  active: boolean;
  onClick: () => void;
  children: React.ReactNode;
}): JSX.Element {
  return (
    <button
      type="button"
      className={"chip" + (active ? " chip--active" : "")}
      onClick={onClick}
    >
      {children}
    </button>
  );
}
