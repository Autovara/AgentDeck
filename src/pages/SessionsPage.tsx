import type { JSX } from "react";
import { SessionsTable } from "../components/SessionsTable";
import type { ProjectTagsReport, SessionsReport } from "../lib/tauri";

type AsyncState<T> =
  | { kind: "loading" }
  | { kind: "ready"; report: T }
  | { kind: "error"; message: string };

type Props = {
  sessions: AsyncState<SessionsReport>;
  tags: AsyncState<ProjectTagsReport>;
  busyTag: string | null;
  onAssignTag: (sessionId: string, tagName: string) => Promise<void> | void;
  onClearTag: (sessionId: string) => Promise<void> | void;
};

export function SessionsPage(props: Props): JSX.Element {
  return (
    <div className="page">
      <header className="page__header">
        <h1>Sessions</h1>
        <p className="page__lead">
          Every agent session AgentDeck has tracked, oldest first. Edit a
          row's tag to attribute its cost; the dropdown is populated from
          the Project Tags card on the Settings page.
        </p>
      </header>

      <div className="page__grid">
        <section className="card">
          <header className="card__header">
            <h2>All sessions</h2>
            <Pill sessions={props.sessions} />
          </header>
          {props.sessions.kind === "loading" && (
            <p className="card__body">Loading sessions…</p>
          )}
          {props.sessions.kind === "error" && (
            <div className="card__body card__body--error">
              <pre className="card__diagnostic">{props.sessions.message}</pre>
            </div>
          )}
          {props.sessions.kind === "ready" && (
            <div className="card__body">
              {props.sessions.report.error != null && (
                <div
                  className="card__body--error"
                  style={{ marginBottom: 12 }}
                >
                  <pre className="card__diagnostic">
                    {props.sessions.report.error}
                  </pre>
                </div>
              )}
              <SessionsTable
                sessions={props.sessions.report.sessions}
                tags={tagsOrEmpty(props.tags)}
                busyTag={props.busyTag}
                onAssignTag={props.onAssignTag}
                onClearTag={props.onClearTag}
              />
            </div>
          )}
        </section>
      </div>
    </div>
  );
}

function Pill({ sessions }: { sessions: Props["sessions"] }): JSX.Element {
  if (sessions.kind === "loading")
    return <span className="pill pill--neutral">Loading</span>;
  if (sessions.kind === "error")
    return <span className="pill pill--warn">Unreachable</span>;
  if (!sessions.report.ready)
    return <span className="pill pill--warn">Offline</span>;
  const total = sessions.report.sessions.length;
  return <span className="pill pill--ok">{total} total</span>;
}

function tagsOrEmpty(
  state: AsyncState<ProjectTagsReport>,
): ProjectTagsReport["tags"] {
  if (state.kind === "ready") return state.report.tags;
  return [];
}
