import type { JSX } from "react";
import type {
  AdapterDiagnostic,
  AdapterDiagnosticsReport,
  CapabilityLevel,
  Confidence,
} from "../lib/tauri";

type Props = {
  state:
    | { kind: "loading" }
    | { kind: "ready"; report: AdapterDiagnosticsReport }
    | { kind: "error"; message: string };
};

export function AdapterDiagnosticsCard({ state }: Props): JSX.Element {
  return (
    <section className="card">
      <header className="card__header">
        <h2>Adapter diagnostics</h2>
        <StatusPill state={state} />
      </header>

      {state.kind === "loading" && (
        <p className="card__body">Reading diagnostics…</p>
      )}

      {state.kind === "error" && (
        <div className="card__body card__body--error">
          <p>
            <strong>Could not load adapter diagnostics.</strong>
          </p>
          <pre className="card__diagnostic">{state.message}</pre>
        </div>
      )}

      {state.kind === "ready" && <Body report={state.report} />}
    </section>
  );
}

function StatusPill({ state }: Props): JSX.Element {
  if (state.kind === "loading") {
    return <span className="pill pill--neutral">Loading</span>;
  }
  if (state.kind === "error") {
    return <span className="pill pill--warn">Unreachable</span>;
  }
  if (!state.report.ready) {
    return <span className="pill pill--warn">Unavailable</span>;
  }
  const failing = state.report.items.filter(
    (i) => i.failureReasons.length > 0,
  ).length;
  if (failing > 0) {
    return <span className="pill pill--warn">{failing} failing</span>;
  }
  if (state.report.items.length === 0) {
    return <span className="pill pill--neutral">No scans yet</span>;
  }
  return (
    <span className="pill pill--ok">{state.report.items.length} adapters</span>
  );
}

function Body({ report }: { report: AdapterDiagnosticsReport }): JSX.Element {
  if (report.error != null) {
    return (
      <div className="card__body card__body--error">
        <pre className="card__diagnostic">{report.error}</pre>
      </div>
    );
  }

  if (report.items.length === 0) {
    return (
      <div className="card__body">
        <p className="card__hint">
          No adapter diagnostics have been recorded yet. Click{" "}
          <strong>Refresh now</strong> in the sidebar to drive one monitor
          tick — the registry will persist one row per registered adapter.
        </p>
      </div>
    );
  }

  return (
    <div className="card__body">
      <table className="table">
        <thead>
          <tr>
            <th>Adapter</th>
            <th>Status</th>
            <th>Level</th>
            <th>Confidence</th>
            <th style={{ textAlign: "right" }}>Detected</th>
            <th>Last scan</th>
          </tr>
        </thead>
        <tbody>
          {report.items.map((item) => (
            <Row key={item.adapterName} item={item} />
          ))}
        </tbody>
      </table>
      <p className="card__hint">
        Captured at {formatTimestamp(report.capturedAt)}. Only the most
        recent scan per adapter is kept; previous scans are overwritten.
      </p>
    </div>
  );
}

function Row({ item }: { item: AdapterDiagnostic }): JSX.Element {
  const hasDetail =
    item.dataSourcesUsed.length +
      item.missingPermissions.length +
      item.failureReasons.length +
      item.knownLimitations.length >
    0;
  return (
    <>
      <tr>
        <td>{item.adapterName}</td>
        <td>
          {item.enabled ? (
            <span className="pill pill--ok">enabled</span>
          ) : (
            <span className="pill pill--neutral">disabled</span>
          )}
        </td>
        <td>{capabilityLabel(item.capabilityLevel)}</td>
        <td>{confidenceLabel(item.confidence)}</td>
        <td style={{ textAlign: "right" }}>{item.detectedCount}</td>
        <td title={item.lastScanTime}>{formatAge(item.lastScanTime)} ago</td>
      </tr>
      {hasDetail && (
        <tr>
          <td colSpan={6} className="adapter-diag__detail">
            <details>
              <summary>Details</summary>
              <div className="adapter-diag__detail-grid">
                <DetailList
                  label="Data sources"
                  values={item.dataSourcesUsed}
                />
                <DetailList
                  label="Missing permissions"
                  values={item.missingPermissions}
                  {...(item.missingPermissions.length > 0
                    ? { tone: "warn" as const }
                    : {})}
                />
                <DetailList
                  label="Failure reasons"
                  values={item.failureReasons}
                  {...(item.failureReasons.length > 0
                    ? { tone: "bad" as const }
                    : {})}
                />
                <DetailList
                  label="Known limitations"
                  values={item.knownLimitations}
                />
              </div>
            </details>
          </td>
        </tr>
      )}
    </>
  );
}

function DetailList({
  label,
  values,
  tone,
}: {
  label: string;
  values: string[];
  tone?: "warn" | "bad";
}): JSX.Element {
  const className =
    "adapter-diag__detail-list" +
    (tone != null ? ` adapter-diag__detail-list--${tone}` : "");
  return (
    <div className={className}>
      <div className="adapter-diag__detail-label">{label}</div>
      {values.length === 0 ? (
        <div className="adapter-diag__detail-empty">none</div>
      ) : (
        <ul>
          {values.map((v, i) => (
            <li key={i}>{v}</li>
          ))}
        </ul>
      )}
    </div>
  );
}

function capabilityLabel(level: CapabilityLevel): string {
  switch (level) {
    case "presence":
      return "L1 · presence";
    case "status":
      return "L2 · status";
    case "usage":
      return "L3 · usage";
    case "control":
      return "L4 · control";
  }
}

function confidenceLabel(c: Confidence): string {
  return c.charAt(0).toUpperCase() + c.slice(1);
}

function formatTimestamp(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleString();
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
