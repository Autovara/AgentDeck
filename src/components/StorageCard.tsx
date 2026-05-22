import type { JSX } from "react";
import type { StorageReport } from "../lib/tauri";

type Props = {
  state:
    | { kind: "loading" }
    | { kind: "ready"; report: StorageReport }
    | { kind: "error"; message: string };
};

export function StorageCard({ state }: Props): JSX.Element {
  return (
    <section className="card">
      <header className="card__header">
        <h2>Storage</h2>
        <StatusPill state={state} />
      </header>

      {state.kind === "loading" && (
        <p className="card__body">Opening the database…</p>
      )}

      {state.kind === "error" && (
        <div className="card__body card__body--error">
          <p>
            <strong>Could not query storage.</strong> The Tauri command itself
            failed; the database may or may not be open.
          </p>
          <pre className="card__diagnostic">{state.message}</pre>
        </div>
      )}

      {state.kind === "ready" && <StorageDetails report={state.report} />}
    </section>
  );
}

function StatusPill({ state }: Props): JSX.Element {
  if (state.kind === "loading") {
    return <span className="pill pill--neutral">Probing</span>;
  }
  if (state.kind === "error") {
    return <span className="pill pill--warn">Unreachable</span>;
  }
  if (!state.report.ready) {
    return <span className="pill pill--warn">Failed</span>;
  }
  return <span className="pill pill--ok">Ready</span>;
}

function StorageDetails({ report }: { report: StorageReport }): JSX.Element {
  return (
    <div className="card__body">
      <dl className="kv">
        <Row label="Path" value={report.path} />
        <Row
          label="Schema"
          value={
            report.schemaVersion == null
              ? "—"
              : `v${report.schemaVersion} (${report.appliedMigrations.length} migrations applied)`
          }
        />
        <Row
          label="Journal"
          value={report.journalMode?.toUpperCase() ?? "—"}
        />
        <Row label="Size" value={formatSize(report.sizeBytes)} />
        <Row label="Captured" value={formatTimestamp(report.capturedAt)} />
      </dl>

      {report.error != null && (
        <div className="card__body--error" style={{ marginTop: 12 }}>
          <pre className="card__diagnostic">{report.error}</pre>
        </div>
      )}

      {report.tables.length > 0 && (
        <details className="card__details">
          <summary>
            Tables ({report.tables.length}, total rows{" "}
            {report.tables.reduce((sum, t) => sum + t.rowCount, 0)})
          </summary>
          <table className="table">
            <thead>
              <tr>
                <th>Table</th>
                <th style={{ textAlign: "right" }}>Rows</th>
              </tr>
            </thead>
            <tbody>
              {report.tables.map((t) => (
                <tr key={t.name}>
                  <td>{t.name}</td>
                  <td style={{ textAlign: "right" }}>{t.rowCount}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </details>
      )}
    </div>
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

function formatSize(bytes: number | null): string {
  if (bytes == null) {
    return "—";
  }
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  if (bytes < 1024 * 1024) {
    return `${(bytes / 1024).toFixed(1)} KiB`;
  }
  if (bytes < 1024 * 1024 * 1024) {
    return `${(bytes / (1024 * 1024)).toFixed(1)} MiB`;
  }
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GiB`;
}

function formatTimestamp(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) {
    return iso;
  }
  return d.toLocaleString();
}
