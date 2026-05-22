import { useCallback, useEffect, useState } from "react";
import type { JSX } from "react";
import {
  addCustomAdapter,
  deleteCustomAdapter,
  getAttentionReport,
  getCustomAdapterReport,
  getProcessScannerReport,
  getStorageReport,
  getTraySurface,
  muteAttentionItem,
  resolveAttentionItem,
  runMonitorTick,
  setCustomAdapterEnabled,
} from "./lib/tauri";
import type {
  AttentionReport,
  CustomAdapterReport,
  NewCustomAdapter,
  ProcessScannerReport,
  StorageReport,
  TraySurfaceReport,
} from "./lib/tauri";
import { TraySurfaceCard } from "./components/TraySurfaceCard";
import { StorageCard } from "./components/StorageCard";
import { ProcessScannerCard } from "./components/ProcessScannerCard";
import { CustomAdaptersCard } from "./components/CustomAdaptersCard";
import { AttentionCard } from "./components/AttentionCard";

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
  const [scanner, setScanner] = useState<AsyncState<ProcessScannerReport>>({
    kind: "loading",
  });
  const [rescanning, setRescanning] = useState(false);
  const [custom, setCustom] = useState<AsyncState<CustomAdapterReport>>({
    kind: "loading",
  });
  const [customBusy, setCustomBusy] = useState(false);
  const [attention, setAttention] = useState<AsyncState<AttentionReport>>({
    kind: "loading",
  });
  const [ticking, setTicking] = useState(false);
  const [attentionBusy, setAttentionBusy] = useState(false);

  const refreshScanner = useCallback(async (): Promise<void> => {
    setRescanning(true);
    try {
      const report = await getProcessScannerReport();
      setScanner({ kind: "ready", report });
    } catch (err) {
      setScanner({ kind: "error", message: describeError(err) });
    } finally {
      setRescanning(false);
    }
  }, []);

  const refreshCustom = useCallback(async (): Promise<void> => {
    try {
      const report = await getCustomAdapterReport();
      setCustom({ kind: "ready", report });
    } catch (err) {
      setCustom({ kind: "error", message: describeError(err) });
    }
  }, []);

  const handleAddCustom = useCallback(
    async (input: NewCustomAdapter): Promise<void> => {
      setCustomBusy(true);
      try {
        const report = await addCustomAdapter(input);
        setCustom({ kind: "ready", report });
      } finally {
        setCustomBusy(false);
      }
    },
    [],
  );

  const handleDeleteCustom = useCallback(async (id: string): Promise<void> => {
    setCustomBusy(true);
    try {
      const report = await deleteCustomAdapter(id);
      setCustom({ kind: "ready", report });
    } catch (err) {
      setCustom({ kind: "error", message: describeError(err) });
    } finally {
      setCustomBusy(false);
    }
  }, []);

  const handleToggleCustom = useCallback(
    async (id: string, enabled: boolean): Promise<void> => {
      setCustomBusy(true);
      try {
        const report = await setCustomAdapterEnabled(id, enabled);
        setCustom({ kind: "ready", report });
      } catch (err) {
        setCustom({ kind: "error", message: describeError(err) });
      } finally {
        setCustomBusy(false);
      }
    },
    [],
  );

  const refreshAttention = useCallback(async (): Promise<void> => {
    try {
      const report = await getAttentionReport();
      setAttention({ kind: "ready", report });
    } catch (err) {
      setAttention({ kind: "error", message: describeError(err) });
    }
  }, []);

  const handleRunTick = useCallback(async (): Promise<void> => {
    setTicking(true);
    try {
      await runMonitorTick();
      await refreshAttention();
      // The scanner card mirrors the same snapshot, so refresh it too.
      await refreshScanner();
    } finally {
      setTicking(false);
    }
  }, [refreshAttention, refreshScanner]);

  const handleMuteAttention = useCallback(
    async (id: string, hours: number): Promise<void> => {
      setAttentionBusy(true);
      try {
        await muteAttentionItem(id, hours);
        await refreshAttention();
      } catch (err) {
        setAttention({ kind: "error", message: describeError(err) });
      } finally {
        setAttentionBusy(false);
      }
    },
    [refreshAttention],
  );

  const handleResolveAttention = useCallback(
    async (id: string): Promise<void> => {
      setAttentionBusy(true);
      try {
        await resolveAttentionItem(id);
        await refreshAttention();
      } catch (err) {
        setAttention({ kind: "error", message: describeError(err) });
      } finally {
        setAttentionBusy(false);
      }
    },
    [refreshAttention],
  );

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
    void refreshScanner();
    void refreshCustom();
    void refreshAttention();
    return () => {
      cancelled = true;
    };
  }, [refreshScanner, refreshCustom, refreshAttention]);

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
        <ProcessScannerCard
          state={scanner}
          onRescan={() => {
            void refreshScanner();
          }}
          rescanning={rescanning}
        />
        <CustomAdaptersCard
          state={custom}
          busy={customBusy}
          onAdd={handleAddCustom}
          onDelete={handleDeleteCustom}
          onToggle={handleToggleCustom}
        />
        <AttentionCard
          state={attention}
          ticking={ticking}
          busy={attentionBusy}
          onRunTick={() => {
            void handleRunTick();
          }}
          onMute={(id, hours) => {
            void handleMuteAttention(id, hours);
          }}
          onResolve={(id) => {
            void handleResolveAttention(id);
          }}
        />

        <section className="card card--placeholder">
          <h2>Active sessions</h2>
          <p>
            The dedicated sessions card lands with build-plan §15 step 8.
            Until then, recent activity is visible through{" "}
            <em>Run monitor tick</em> and the Attention card above.
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
