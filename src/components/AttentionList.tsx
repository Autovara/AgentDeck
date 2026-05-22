import type { JSX } from "react";
import type {
  AttentionSeverity,
  OpenAttentionEntry,
  RecommendedAction,
} from "../lib/tauri";

type Props = {
  /** Already-filtered entries to render. */
  entries: OpenAttentionEntry[];
  /** True while a per-item mutation is in flight. */
  busy: boolean;
  onMute: (id: string, hours: number) => void;
  onResolve: (id: string) => void;
  /** Optional cap. When set, only the first `limit` entries are rendered
   * and a "+N more" caption appears underneath. */
  limit?: number;
  /** Optional message shown when `entries` is empty. */
  emptyMessage?: JSX.Element | string;
  /** Optional list of columns to render. Defaults to the full set. */
  columns?: ColumnConfig;
};

export type ColumnConfig = {
  severity?: boolean;
  reason?: boolean;
  session?: boolean;
  repo?: boolean;
  confidence?: boolean;
  message?: boolean;
  age?: boolean;
  actions?: boolean;
};

const DEFAULT_COLUMNS: Required<ColumnConfig> = {
  severity: true,
  reason: true,
  session: true,
  repo: false,
  confidence: false,
  message: true,
  age: true,
  actions: true,
};

export function AttentionList({
  entries,
  busy,
  onMute,
  onResolve,
  limit,
  emptyMessage,
  columns,
}: Props): JSX.Element {
  const cols = { ...DEFAULT_COLUMNS, ...(columns ?? {}) };

  if (entries.length === 0) {
    return (
      <p className="card__hint">
        {emptyMessage ?? "No attention items right now."}
      </p>
    );
  }

  const shown = limit != null ? entries.slice(0, limit) : entries;
  const hiddenCount = limit != null ? Math.max(0, entries.length - limit) : 0;

  return (
    <>
      <table className="table">
        <thead>
          <tr>
            {cols.severity && <th>Severity</th>}
            {cols.reason && <th>Reason</th>}
            {cols.session && <th>Session</th>}
            {cols.repo && <th>Repo</th>}
            {cols.confidence && <th>Confidence</th>}
            {cols.message && <th>Message</th>}
            {cols.age && <th>Age</th>}
            {cols.actions && <th style={{ textAlign: "right" }}>Actions</th>}
          </tr>
        </thead>
        <tbody>
          {shown.map((entry) => (
            <Row
              key={entry.item.id}
              entry={entry}
              busy={busy}
              onMute={onMute}
              onResolve={onResolve}
              cols={cols}
            />
          ))}
        </tbody>
      </table>
      {hiddenCount > 0 && (
        <p className="card__hint" style={{ marginTop: 8 }}>
          + {hiddenCount} more on the Attention page.
        </p>
      )}
    </>
  );
}

function Row({
  entry,
  busy,
  onMute,
  onResolve,
  cols,
}: {
  entry: OpenAttentionEntry;
  busy: boolean;
  onMute: Props["onMute"];
  onResolve: Props["onResolve"];
  cols: Required<ColumnConfig>;
}): JSX.Element {
  const { item, session } = entry;
  const muted = item.mutedUntil != null && new Date(item.mutedUntil) > new Date();
  const allowsStop = item.recommendedActions.includes(
    "stop" as RecommendedAction,
  );
  return (
    <tr style={{ opacity: muted ? 0.65 : 1 }}>
      {cols.severity && (
        <td>
          <SeverityPill severity={item.severity} />
          {muted && (
            <span className="pill pill--neutral" style={{ marginLeft: 6 }}>
              muted
            </span>
          )}
        </td>
      )}
      {cols.reason && <td>{item.reason}</td>}
      {cols.session && (
        <td>
          {session.agentName}
          {session.pid != null && (
            <span className="card__hint" style={{ marginLeft: 4 }}>
              pid {session.pid}
            </span>
          )}
        </td>
      )}
      {cols.repo && (
        <td className="attention-list__repo" title={repoLabel(entry).tooltip}>
          {repoLabel(entry).short}
        </td>
      )}
      {cols.confidence && <td>{item.confidence}</td>}
      {cols.message && <td>{item.message}</td>}
      {cols.age && <td>{formatAge(item.createdAt)}</td>}
      {cols.actions && (
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
      )}
    </tr>
  );
}

function repoLabel(entry: OpenAttentionEntry): {
  short: string;
  tooltip: string;
} {
  const { repoPath, projectTag } = entry.session;
  if (projectTag != null && projectTag !== "") {
    return { short: projectTag, tooltip: projectTag };
  }
  if (repoPath != null && repoPath !== "") {
    const parts = repoPath.split(/[\\/]/).filter((p) => p !== "");
    const last = parts.length > 0 ? parts[parts.length - 1] : undefined;
    return { short: last ?? repoPath, tooltip: repoPath };
  }
  return { short: "—", tooltip: "no repo path" };
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
