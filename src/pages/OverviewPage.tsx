import type { JSX } from "react";
import { AttentionList } from "../components/AttentionList";
import { SummaryTile } from "../components/SummaryTile";
import type { OverviewReport } from "../lib/tauri";

type State =
  | { kind: "loading" }
  | { kind: "ready"; report: OverviewReport }
  | { kind: "error"; message: string };

type Props = {
  state: State;
  busy: boolean;
  onMute: (id: string, hours: number) => void;
  onResolve: (id: string) => void;
  onOpenAttention: () => void;
};

export function OverviewPage({
  state,
  busy,
  onMute,
  onResolve,
  onOpenAttention,
}: Props): JSX.Element {
  return (
    <section className="page page--overview">
      <header className="page__header">
        <div>
          <h1 className="page__title">Overview</h1>
          <p className="page__subtitle">
            What the monitor is currently watching and what it wants you to
            look at.
          </p>
        </div>
      </header>

      {state.kind === "loading" && (
        <div className="card">
          <p className="card__body">Loading overview…</p>
        </div>
      )}

      {state.kind === "error" && (
        <div className="card">
          <div className="card__body card__body--error">
            <p>
              <strong>Overview unreachable.</strong>
            </p>
            <pre className="card__diagnostic">{state.message}</pre>
          </div>
        </div>
      )}

      {state.kind === "ready" && (
        <OverviewBody
          report={state.report}
          busy={busy}
          onMute={onMute}
          onResolve={onResolve}
          onOpenAttention={onOpenAttention}
        />
      )}
    </section>
  );
}

function OverviewBody({
  report,
  busy,
  onMute,
  onResolve,
  onOpenAttention,
}: {
  report: OverviewReport;
  busy: boolean;
  onMute: Props["onMute"];
  onResolve: Props["onResolve"];
  onOpenAttention: () => void;
}): JSX.Element {
  return (
    <div className="page__body">
      <div className="summary-grid">
        <SummaryTile
          label="Active agents"
          value={report.activeSessions}
          caption={
            report.activeSessions === 0
              ? "No active sessions in this snapshot"
              : pluralCaption(report.activeSessions, "session", "sessions")
          }
          kind="neutral"
        />
        <SummaryTile
          label="Open attention"
          value={report.attentionTotal}
          caption={attentionCaption(report)}
          kind={attentionKind(report)}
        />
        <SummaryTile
          label="Stalled / waiting"
          value={`${report.stalledSessions} / ${report.waitingSessions}`}
          caption={
            <>
              stalled: {report.stalledSessions}
              <br />
              waiting for input: {report.waitingSessions}
            </>
          }
          kind={report.stalledSessions > 0 ? "warn" : "neutral"}
        />
        <SummaryTile
          label="Estimated cost today"
          value={
            report.estimatedCostToday == null
              ? "—"
              : `$${report.estimatedCostToday.toFixed(2)}`
          }
          caption={
            report.estimatedCostToday == null
              ? "No cost data yet (lands in §15 step 18)"
              : "Includes only sessions started since local midnight"
          }
          kind="neutral"
        />
      </div>

      {report.error != null && (
        <div className="card">
          <div className="card__body card__body--error">
            <pre className="card__diagnostic">{report.error}</pre>
          </div>
        </div>
      )}

      <section className="card">
        <header className="card__header">
          <h2>Recent attention</h2>
          <div className="card__header-actions">
            <button
              type="button"
              className="card__button"
              onClick={onOpenAttention}
            >
              Open Attention page →
            </button>
          </div>
        </header>
        <div className="card__body">
          <AttentionList
            entries={report.recentAttention}
            busy={busy}
            onMute={onMute}
            onResolve={onResolve}
            limit={5}
            columns={{
              severity: true,
              reason: true,
              session: true,
              message: true,
              age: true,
              actions: true,
            }}
            emptyMessage={
              <>
                Nothing is asking for attention right now. The monitor will
                surface a row here when a session enters{" "}
                <code>rate_limited</code>, <code>errored</code>,{" "}
                <code>stalled</code>, or (for Level&nbsp;2+ adapters){" "}
                <code>waiting_for_input</code>.
              </>
            }
          />
        </div>
      </section>
    </div>
  );
}

function pluralCaption(n: number, singular: string, plural: string): string {
  return `${n} ${n === 1 ? singular : plural}`;
}

function attentionCaption(report: OverviewReport): string {
  if (report.attentionTotal === 0) {
    return "All clear";
  }
  const parts: string[] = [];
  if (report.attentionUrgent > 0) parts.push(`${report.attentionUrgent} urgent`);
  if (report.attentionWarn > 0) parts.push(`${report.attentionWarn} warn`);
  if (report.attentionInfo > 0) parts.push(`${report.attentionInfo} info`);
  return parts.join(" · ");
}

function attentionKind(report: OverviewReport): "ok" | "warn" | "bad" | "neutral" {
  if (report.attentionUrgent > 0) return "bad";
  if (report.attentionWarn > 0) return "warn";
  if (report.attentionTotal > 0) return "neutral";
  return "ok";
}
