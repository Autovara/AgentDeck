import type { JSX } from "react";
import type { ProjectTag, SessionRow } from "../lib/tauri";

type Props = {
  sessions: SessionRow[];
  tags: ProjectTag[];
  busyTag: string | null;
  onAssignTag: (sessionId: string, tagName: string) => Promise<void> | void;
  onClearTag: (sessionId: string) => Promise<void> | void;
};

const NO_TAG = "__none__";

export function SessionsTable({
  sessions,
  tags,
  busyTag,
  onAssignTag,
  onClearTag,
}: Props): JSX.Element {
  if (sessions.length === 0) {
    return (
      <p className="card__hint">
        No sessions yet. Sessions appear as soon as the monitor tick finds an
        agent process.
      </p>
    );
  }

  return (
    <table className="table">
      <thead>
        <tr>
          <th>Agent</th>
          <th>Status</th>
          <th>Repo</th>
          <th>Tag</th>
          <th>Runtime</th>
          <th>Cost</th>
        </tr>
      </thead>
      <tbody>
        {sessions.map((s) => (
          <tr
            key={s.id}
            style={s.status === "completed" ? { opacity: 0.6 } : undefined}
          >
            <td>
              <div title={s.command}>{s.agentName}</div>
              <div className="card__hint" style={{ fontSize: "0.75rem" }}>
                [{shortId(s.id)}] · {s.adapterName}
              </div>
            </td>
            <td>{s.status}</td>
            <td title={s.repoPath ?? undefined}>{repoLabel(s.repoPath)}</td>
            <td>
              <select
                value={s.projectTag ?? NO_TAG}
                disabled={busyTag === s.id}
                onChange={(e) => {
                  const next = e.target.value;
                  if (next === NO_TAG) {
                    void onClearTag(s.id);
                  } else {
                    void onAssignTag(s.id, next);
                  }
                }}
              >
                <option value={NO_TAG}>(none)</option>
                {tags.map((t) => (
                  <option key={t.id} value={t.name}>
                    {t.name}
                  </option>
                ))}
                {/* If the session has a tag that no longer exists in the
                    catalog, still surface it so the row stays usable. */}
                {s.projectTag != null &&
                  !tags.some((t) => t.name === s.projectTag) && (
                    <option value={s.projectTag}>
                      {s.projectTag} (deleted)
                    </option>
                  )}
              </select>
            </td>
            <td>{runtimeLabel(s.startTime, s.lastSeenTime)}</td>
            <td title={costTooltip(s)}>{costLabel(s)}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

function shortId(uuid: string): string {
  return uuid.replace(/-/g, "").slice(0, 6);
}

function repoLabel(repo: string | null): string {
  if (repo == null) return "—";
  const last = repo.split(/[\\/]/).pop();
  return last && last.length > 0 ? last : repo;
}

function runtimeLabel(startIso: string, lastSeenIso: string): string {
  const start = new Date(startIso).getTime();
  const end = new Date(lastSeenIso).getTime();
  if (Number.isNaN(start) || Number.isNaN(end)) return "—";
  const secs = Math.max(0, Math.floor((end - start) / 1000));
  if (secs < 60) return `${secs}s`;
  const mins = Math.floor(secs / 60);
  if (mins < 60) return `${mins}m`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) return `${hours}h ${mins % 60}m`;
  const days = Math.floor(hours / 24);
  return `${days}d ${hours % 24}h`;
}

function costLabel(s: SessionRow): string {
  if (s.estimatedCost == null) return "—";
  const formatted = `$${s.estimatedCost.toFixed(2)}`;
  switch (s.costKind) {
    case "exact":
      return formatted;
    case "estimated":
      return `~${formatted}`;
    case "unknown":
      return formatted;
  }
}

function costTooltip(s: SessionRow): string | undefined {
  if (s.estimatedCost == null) {
    return "No rate configured for this adapter.";
  }
  return `cost_kind = ${s.costKind}`;
}
