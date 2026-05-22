import type { JSX } from "react";
import type { TraySurfaceReport, TrayMechanism } from "../lib/tauri";

type Props = {
  state:
    | { kind: "loading" }
    | { kind: "ready"; report: TraySurfaceReport }
    | { kind: "error"; message: string };
};

export function TraySurfaceCard({ state }: Props): JSX.Element {
  return (
    <section className="card">
      <header className="card__header">
        <h2>Tray surface</h2>
        <StatusPill state={state} />
      </header>

      {state.kind === "loading" && (
        <p className="card__body">Probing the host environment…</p>
      )}

      {state.kind === "error" && (
        <div className="card__body card__body--error">
          <p>
            <strong>Tray probe failed.</strong> The dashboard is running, but
            we could not determine whether a tray surface is available.
          </p>
          <pre className="card__diagnostic">{state.message}</pre>
        </div>
      )}

      {state.kind === "ready" && <TrayReport report={state.report} />}
    </section>
  );
}

function StatusPill({ state }: Props): JSX.Element {
  if (state.kind === "loading") {
    return <span className="pill pill--neutral">Probing</span>;
  }
  if (state.kind === "error") {
    return <span className="pill pill--warn">Probe failed</span>;
  }
  if (state.report.available) {
    return <span className="pill pill--ok">Tray active</span>;
  }
  return <span className="pill pill--warn">Dashboard fallback</span>;
}

function TrayReport({ report }: { report: TraySurfaceReport }): JSX.Element {
  return (
    <div className="card__body">
      <dl className="kv">
        <Row label="Environment" value={report.environment} />
        <Row label="Mechanism" value={describeMechanism(report.mechanism)} />
        {report.desktopEnvironment != null && (
          <Row label="Desktop" value={report.desktopEnvironment} />
        )}
        {report.sessionType != null && (
          <Row label="Session" value={report.sessionType} />
        )}
        <Row label="Reason" value={report.reason} />
        <Row
          label="Surface"
          value={
            report.fallbackRequired
              ? "Dashboard window (tray unavailable)"
              : "Tray icon"
          }
        />
        <Row
          label="Probed at"
          value={formatTimestamp(report.probedAt)}
        />
      </dl>

      {report.fallbackRequired && report.helpUrl != null && (
        <p className="card__hint">
          On {report.desktopEnvironment ?? "this environment"} a tray icon may
          be possible after installing an extension or panel applet.{" "}
          <a href={report.helpUrl} target="_blank" rel="noreferrer">
            Setup instructions
          </a>
          .
        </p>
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

function describeMechanism(mechanism: TrayMechanism): string {
  switch (mechanism) {
    case "macos_status_item":
      return "macOS NSStatusItem";
    case "windows_shell_notify_icon":
      return "Windows Shell_NotifyIcon";
    case "linux_status_notifier":
      return "StatusNotifierItem (SNI)";
    case "linux_xembed":
      return "XEmbed legacy tray";
    case "linux_appindicator":
      return "Ayatana AppIndicator";
    case "unsupported":
      return "Not supported on this host";
    case "unknown":
      return "Unknown";
  }
}

function formatTimestamp(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) {
    return iso;
  }
  return d.toLocaleString();
}
