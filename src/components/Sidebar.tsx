import type { JSX } from "react";

export type PageId =
  | "overview"
  | "attention"
  | "sessions"
  | "diagnostics"
  | "settings";

export type SidebarItem = {
  id: PageId;
  label: string;
  /** Optional badge text shown on the right side of the row. */
  badge?: string | null;
  /** Optional badge severity ("warn" / "bad" / "ok" / "neutral"). */
  badgeKind?: "warn" | "bad" | "ok" | "neutral";
};

type Props = {
  items: SidebarItem[];
  activeId: PageId;
  onSelect: (id: PageId) => void;
  /** Sidebar footer "Refresh now" button (drives `run_monitor_tick`). */
  refreshing: boolean;
  onRefresh: () => void;
  /** Optional ISO timestamp of the most recent tick. */
  lastTickAt: string | null;
};

export function Sidebar({
  items,
  activeId,
  onSelect,
  refreshing,
  onRefresh,
  lastTickAt,
}: Props): JSX.Element {
  return (
    <aside className="sidebar" aria-label="Primary navigation">
      <div className="sidebar__brand">
        <span className="sidebar__brand-mark" aria-hidden>
          ▣
        </span>
        <div>
          <h1 className="sidebar__brand-name">AgentDeck</h1>
          <p className="sidebar__brand-tagline">attention board · alpha</p>
        </div>
      </div>

      <nav className="sidebar__nav" role="navigation">
        <ul>
          {items.map((item) => (
            <li key={item.id}>
              <button
                type="button"
                className={
                  "sidebar__nav-item" +
                  (item.id === activeId ? " sidebar__nav-item--active" : "")
                }
                aria-current={item.id === activeId ? "page" : undefined}
                onClick={() => onSelect(item.id)}
              >
                <span className="sidebar__nav-label">{item.label}</span>
                {item.badge != null && (
                  <span
                    className={
                      "sidebar__badge sidebar__badge--" +
                      (item.badgeKind ?? "neutral")
                    }
                  >
                    {item.badge}
                  </span>
                )}
              </button>
            </li>
          ))}
        </ul>
      </nav>

      <div className="sidebar__footer">
        <button
          type="button"
          className="card__button sidebar__refresh"
          disabled={refreshing}
          onClick={onRefresh}
          title="Run one monitor tick: snapshot → adapters → sessions → attention"
        >
          {refreshing ? "Refreshing…" : "Refresh now"}
        </button>
        <p className="sidebar__hint">
          {lastTickAt != null
            ? `Last tick ${formatRelative(lastTickAt)}`
            : "No tick yet this session"}
        </p>
        <p className="sidebar__hint sidebar__hint--muted">
          Alpha pre-release — not for production use.
        </p>
      </div>
    </aside>
  );
}

function formatRelative(iso: string): string {
  try {
    const then = new Date(iso).getTime();
    const diff = Math.max(0, Date.now() - then);
    const seconds = Math.floor(diff / 1000);
    if (seconds < 5) return "just now";
    if (seconds < 60) return `${seconds}s ago`;
    const minutes = Math.floor(seconds / 60);
    if (minutes < 60) return `${minutes}m ago`;
    const hours = Math.floor(minutes / 60);
    if (hours < 24) return `${hours}h ago`;
    return `${Math.floor(hours / 24)}d ago`;
  } catch {
    return iso;
  }
}
