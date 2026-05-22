import type { JSX } from "react";
import type { ProcessScannerReport, AgentCandidate } from "../lib/tauri";

type Props = {
  state:
    | { kind: "loading" }
    | { kind: "ready"; report: ProcessScannerReport }
    | { kind: "error"; message: string };
  /** Called when the user clicks "Rescan". The dashboard wires this to a
   * fresh `getProcessScannerReport()` call. */
  onRescan: () => void;
  /** True while a rescan is in flight; disables the rescan button. */
  rescanning: boolean;
};

export function ProcessScannerCard({
  state,
  onRescan,
  rescanning,
}: Props): JSX.Element {
  return (
    <section className="card">
      <header className="card__header">
        <h2>Process scanner</h2>
        <div className="card__header-actions">
          <StatusPill state={state} />
          <button
            type="button"
            className="card__button"
            onClick={onRescan}
            disabled={rescanning || state.kind === "loading"}
          >
            {rescanning ? "Scanning…" : "Rescan"}
          </button>
        </div>
      </header>

      {state.kind === "loading" && (
        <p className="card__body">Starting the process scanner…</p>
      )}

      {state.kind === "error" && (
        <div className="card__body card__body--error">
          <p>
            <strong>Process scanner unreachable.</strong> The Tauri command
            failed before the Rust side could enumerate processes.
          </p>
          <pre className="card__diagnostic">{state.message}</pre>
        </div>
      )}

      {state.kind === "ready" && <ScannerDetails report={state.report} />}
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
  if (state.report.error != null) {
    return <span className="pill pill--warn">Errored</span>;
  }
  if (!state.report.ready) {
    return <span className="pill pill--neutral">Idle</span>;
  }
  return <span className="pill pill--ok">Ready</span>;
}

function ScannerDetails({
  report,
}: {
  report: ProcessScannerReport;
}): JSX.Element {
  return (
    <div className="card__body">
      <dl className="kv">
        <Row label="Source" value={report.source} />
        <Row
          label="Last scan"
          value={
            report.lastScan == null
              ? "—"
              : `${formatTimestamp(report.lastScan.startedAt)} (${report.lastScan.scanDurationMs} ms)`
          }
        />
        <Row
          label="Processes"
          value={
            report.lastScan == null
              ? "—"
              : `${report.lastScan.totalProcesses.toLocaleString()} visible`
          }
        />
        <Row
          label="Candidates"
          value={
            report.candidates.length === 0
              ? "none matching alpha heuristics"
              : `${report.candidates.length} matched`
          }
        />
      </dl>

      <p className="card__hint">
        Heuristic patterns (alpha, will be replaced by the adapter framework):
        <code>{report.patterns.join(", ")}</code>
      </p>

      {report.error != null && (
        <div className="card__body--error" style={{ marginTop: 12 }}>
          <pre className="card__diagnostic">{report.error}</pre>
        </div>
      )}

      {report.candidates.length > 0 && (
        <details className="card__details" open={report.candidates.length <= 5}>
          <summary>
            Agent candidates ({report.candidates.length})
          </summary>
          <table className="table">
            <thead>
              <tr>
                <th>PID</th>
                <th>Name</th>
                <th>Command line</th>
                <th>Matched</th>
              </tr>
            </thead>
            <tbody>
              {report.candidates.map((c) => (
                <CandidateRow key={c.process.pid} candidate={c} />
              ))}
            </tbody>
          </table>
        </details>
      )}
    </div>
  );
}

function CandidateRow({
  candidate,
}: {
  candidate: AgentCandidate;
}): JSX.Element {
  const cmd = candidate.process.cmdline.join(" ") || candidate.process.name;
  return (
    <tr>
      <td>{candidate.process.pid}</td>
      <td>{candidate.process.name}</td>
      <td title={cmd}>{truncate(cmd, 80)}</td>
      <td>{candidate.matchedPatterns.join(", ")}</td>
    </tr>
  );
}

function Row({ label, value }: { label: string; value: string }): JSX.Element {
  return (
    <>
      <dt>{label}</dt>
      <dd>{value}</dd>
    </>
  );
}

function truncate(s: string, max: number): string {
  if (s.length <= max) {
    return s;
  }
  return `${s.slice(0, max - 1)}…`;
}

function formatTimestamp(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) {
    return iso;
  }
  return d.toLocaleString();
}
