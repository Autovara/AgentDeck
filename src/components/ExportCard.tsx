import { useState } from "react";
import type { JSX } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import { exportSessionsCsv, exportSessionsJson } from "../lib/tauri";
import type { ExportResult } from "../lib/tauri";

type Props = {
  storageReady: boolean;
};

type Last =
  | { kind: "idle" }
  | { kind: "ok"; result: ExportResult }
  | { kind: "error"; message: string };

export function ExportCard({ storageReady }: Props): JSX.Element {
  const [busy, setBusy] = useState(false);
  const [last, setLast] = useState<Last>({ kind: "idle" });

  async function run(format: "csv" | "json"): Promise<void> {
    setBusy(true);
    setLast({ kind: "idle" });
    try {
      const stamp = new Date().toISOString().slice(0, 10);
      const defaultPath = `agentdeck-sessions-${stamp}.${format}`;
      const picked = await save({
        title: "Export sessions",
        defaultPath,
        filters: [
          format === "csv"
            ? { name: "CSV", extensions: ["csv"] }
            : { name: "JSON", extensions: ["json"] },
        ],
      });
      if (picked == null) {
        // User cancelled the dialog. No-op.
        return;
      }
      const result =
        format === "csv"
          ? await exportSessionsCsv(picked)
          : await exportSessionsJson(picked);
      setLast({ kind: "ok", result });
    } catch (err) {
      setLast({ kind: "error", message: describeError(err) });
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="card">
      <header className="card__header">
        <h2>Export</h2>
      </header>
      <div className="card__body">
        <p className="card__hint">
          Save every session (active and completed) to disk. CSV is good for
          spreadsheets; JSON preserves nulls and is easier to script against.
          The same columns are emitted in both formats.
        </p>

        {!storageReady && (
          <p className="card__hint card__body--error">
            Storage is unavailable; export is disabled.
          </p>
        )}

        <div className="custom-adapter-form__actions">
          <button
            type="button"
            className="card__button"
            disabled={busy || !storageReady}
            onClick={() => {
              void run("csv");
            }}
          >
            {busy ? "Exporting…" : "Export CSV"}
          </button>
          <button
            type="button"
            className="card__button"
            disabled={busy || !storageReady}
            style={{ marginLeft: 8 }}
            onClick={() => {
              void run("json");
            }}
          >
            Export JSON
          </button>
        </div>

        {last.kind === "ok" && (
          <p className="card__hint">
            Wrote {last.result.bytesWritten.toLocaleString()} bytes (
            {last.result.format.toUpperCase()}) to{" "}
            <code title={last.result.path}>{last.result.path}</code>.
          </p>
        )}
        {last.kind === "error" && (
          <pre className="card__diagnostic">{last.message}</pre>
        )}
      </div>
    </section>
  );
}

function describeError(err: unknown): string {
  if (err instanceof Error) return err.message;
  if (typeof err === "string") return err;
  return JSON.stringify(err);
}
