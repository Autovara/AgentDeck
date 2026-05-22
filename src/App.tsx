import { useEffect, useState } from "react";
import type { JSX } from "react";
import { getStorageReport, getTraySurface } from "./lib/tauri";
import type { StorageReport, TraySurfaceReport } from "./lib/tauri";
import { TraySurfaceCard } from "./components/TraySurfaceCard";
import { StorageCard } from "./components/StorageCard";

type AsyncState<T> =
  | { kind: "loading" }
  | { kind: "ready"; report: T }
  | { kind: "error"; message: string };

export function App(): JSX.Element {
  const [tray, setTray] = useState<AsyncState<TraySurfaceReport>>({
    kind: "loading",
  });
  const [storage, setStorage] = useState<AsyncState<StorageReport>>({
    kind: "loading",
  });

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const report = await getTraySurface();
        if (!cancelled) {
          setTray({ kind: "ready", report });
        }
      } catch (err) {
        if (!cancelled) {
          setTray({ kind: "error", message: describeError(err) });
        }
      }
    })();
    void (async () => {
      try {
        const report = await getStorageReport();
        if (!cancelled) {
          setStorage({ kind: "ready", report });
        }
      } catch (err) {
        if (!cancelled) {
          setStorage({ kind: "error", message: describeError(err) });
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <div className="app">
      <header className="app__header">
        <h1>AgentDeck</h1>
        <p className="app__subtitle">
          Local attention board for AI coding agents.
        </p>
      </header>

      <main className="app__main">
        <TraySurfaceCard state={tray} />
        <StorageCard state={storage} />

        <section className="card card--placeholder">
          <h2>Active sessions</h2>
          <p>
            No adapters are wired up yet. The process scanner and per-agent
            adapters land in later build steps; until then this surface stays
            empty.
          </p>
        </section>

        <section className="card card--placeholder">
          <h2>Attention</h2>
          <p>
            Once the attention engine is implemented, sessions that need a
            human (waiting for input, rate-limited, stalled, errored) will
            appear here.
          </p>
        </section>
      </main>

      <footer className="app__footer">
        <span>Alpha pre-release — not for production use.</span>
      </footer>
    </div>
  );
}

function describeError(err: unknown): string {
  if (err instanceof Error) {
    return err.message;
  }
  if (typeof err === "string") {
    return err;
  }
  return JSON.stringify(err);
}
