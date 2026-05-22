import type { JSX } from "react";
import type {
  AttentionReport,
  AttentionSeverity,
  OpenAttentionEntry,
  RecommendedAction,
} from "../lib/tauri";

type State =
  | { kind: "loading" }
  | { kind: "ready"; report: AttentionReport }
  | { kind: "error"; message: string };

type Props = {
  state: State;
  /** True while a tick is in flight. */
  ticking: boolean;
  /** True while a per-item mutation is in flight. */
  busy: boolean;
  onRunTick: () => void;
  onMute: (id: string, hours: number) => void;
  onResolve: (id: string) => void;
};

export function AttentionCard({
  state,
  ticking,
  busy,
  onRunTick,
  onMute,
  onResolve,
}: Props): JSX.Element {
  return (
    <section className="card">
      <header className="card__header">
        <h2>Attention</h2>
        <div className="card__header-actions">
          <StatusPill state={state} />
          <button
            type="button"
            className="card__button"
            disabled={ticking}
            onClick={onRunTick}
          >
            {ticking ? "Ticking…" : "Run monitor tick"}
          </button>
        </div>
      </header>

      {state.kind === "loading" && (
        <p className="card__body">Loading attention items…</p>
      )}

      {state.kind === "error" && (
        <div className="card__body card__body--error">
          <p>
            <strong>Attention engine unreachable.</strong>
          </p>
          <pre className="card__diagnostic">{state.message}</pre>
        </div>
      )}

      {state.kind === "ready" && (
        <AttentionDetails
          report={state.report}
          busy={busy}
          onMute={onMute}
          onResolve={onResolve}
        />
      )}
    </section>
  );
}

function StatusPill({ state }: Pick<Props, "state">): JSX.Element {
  if (state.kind === "loading") {
    return <span className="pill pill--neutral">Probing</span>;
  }
  if (state.kind === "error") {
    return <span className="pill pill--warn">Unreachable</span>;
  }
  if (!state.report.ready) {
    return <span className="pill pill--warn">Offline</span>;
  }
  if (state.report.error != null) {
    return <span className="pill pill--warn">Errored</span>;
  }
  const count = state.report.items.length;
  if (count === 0) {
    return <span className="pill pill--ok">All clear</span>;
  }
  const urgent = state.report.items.filter(
    (e) => e.item.severity === "urgent",
  ).length;
  if (urgent > 0) {
    return (
      <span className="pill pill--bad">
        {urgent} urgent / {count}
      </span>
    );
  }
  const warn = state.report.items.filter((e) => e.item.severity === "warn").length;
  if (warn > 0) {
    return (
      <span className="pill pill--warn">
        {warn} warn / {count}
      </span>
    );
  }
  return <span className="pill pill--neutral">{count} info</span>;
}

function AttentionDetails({
  report,
  busy,
  onMute,
  onResolve,
}: {
  report: AttentionReport;
  busy: boolean;
  onMute: Props["onMute"];
  onResolve: Props["onResolve"];
}): JSX.Element {
  return (
    <div className="card__body">
      <p className="card__hint">
        Open attention items the monitor wants you to look at. Click{" "}
        <em>Run monitor tick</em> to scan once and refresh this list. The
        background scheduler lands in a later build step.
      </p>

      {report.error != null && (
        <div className="card__body--error" style={{ marginTop: 12 }}>
          <pre className="card__diagnostic">{report.error}</pre>
        </div>
      )}

      <dl className="kv">
        <dt>Items open</dt>
        <dd>{report.items.length}</dd>
        <dt>Captured at</dt>
        <dd>{formatTimestamp(report.capturedAt)}</dd>
      </dl>

      {report.items.length === 0 ? (
        <p className="card__hint">
          No attention items right now. Items appear here when a session
          enters a state like <code>rate_limited</code>, <code>errored</code>,{" "}
          <code>stalled</code>, or (for Level&nbsp;2+ adapters)
          <code> waiting_for_input</code>.
        </p>
      ) : (
        <table className="table">
          <thead>
            <tr>
              <th>Severity</th>
              <th>Reason</th>
              <th>Session</th>
              <th>Message</th>
              <th>Age</th>
              <th style={{ textAlign: "right" }}>Actions</th>
            </tr>
          </thead>
          <tbody>
            {report.items.map((entry) => (
              <Row
                key={entry.item.id}
                entry={entry}
                busy={busy}
                onMute={onMute}
                onResolve={onResolve}
              />
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}

function Row({
  entry,
  busy,
  onMute,
  onResolve,
}: {
  entry: OpenAttentionEntry;
  busy: boolean;
  onMute: Props["onMute"];
  onResolve: Props["onResolve"];
}): JSX.Element {
  const { item, session } = entry;
  const muted = item.mutedUntil != null && new Date(item.mutedUntil) > new Date();
  const allowsStop = item.recommendedActions.includes(
    "stop" as RecommendedAction,
  );
  return (
    <tr style={{ opacity: muted ? 0.65 : 1 }}>
      <td>
        <SeverityPill severity={item.severity} />
        {muted && (
          <span className="pill pill--neutral" style={{ marginLeft: 6 }}>
            muted
          </span>
        )}
      </td>
      <td>{item.reason}</td>
      <td>
        {session.agentName}
        {session.pid != null && (
          <span className="card__hint" style={{ marginLeft: 4 }}>
            pid {session.pid}
          </span>
        )}
      </td>
      <td>{item.message}</td>
      <td>{formatAge(item.createdAt)}</td>
      <td style={{ textAlign: "right" }}>
        <button
          type="button"
          className="card__button"
          disabled={busy}
          onClick={() => onMute(item.id, muted ? 0 : 1)}
          title={muted ? "Clear mute" : "Mute for 1 hour"}
        >
          {muted ? "Unmute" : "Mute 1h"}
        </button>
        <button
          type="button"
          className="card__button"
          disabled={busy}
          style={{ marginLeft: 4 }}
          onClick={() => onResolve(item.id)}
          title={
            allowsStop
              ? "Mark resolved (does not stop the underlying process)"
              : "Mark resolved"
          }
        >
          Resolve
        </button>
      </td>
    </tr>
  );
}

function SeverityPill({
  severity,
}: {
  severity: AttentionSeverity;
}): JSX.Element {
  switch (severity) {
    case "urgent":
      return <span className="pill pill--bad">urgent</span>;
    case "warn":
      return <span className="pill pill--warn">warn</span>;
    case "info":
    default:
      return <span className="pill pill--neutral">info</span>;
  }
}

function formatTimestamp(iso: string): string {
  try {
    const d = new Date(iso);
    return d.toLocaleString();
  } catch {
    return iso;
  }
}

function formatAge(iso: string): string {
  try {
    const then = new Date(iso).getTime();
    const diff = Math.max(0, Date.now() - then);
    const seconds = Math.floor(diff / 1000);
    if (seconds < 60) return `${seconds}s`;
    const minutes = Math.floor(seconds / 60);
    if (minutes < 60) return `${minutes}m`;
    const hours = Math.floor(minutes / 60);
    if (hours < 24) return `${hours}h`;
    const days = Math.floor(hours / 24);
    return `${days}d`;
  } catch {
    return iso;
  }
}
