import { useEffect, useState } from "react";
import type { JSX } from "react";
import { getTraySurface } from "./lib/tauri";
import type { TraySurfaceReport } from "./lib/tauri";
import { TraySurfaceCard } from "./components/TraySurfaceCard";

type TrayState =
  | { kind: "loading" }
  | { kind: "ready"; report: TraySurfaceReport }
  | { kind: "error"; message: string };

export function App(): JSX.Element {
  const [tray, setTray] = useState<TrayState>({ kind: "loading" });

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
          const message =
            err instanceof Error ? err.message : String(err);
          setTray({ kind: "error", message });
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
