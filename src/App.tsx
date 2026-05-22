import { useCallback, useEffect, useState } from "react";
import type { JSX } from "react";
import {
  addCustomAdapter,
  deleteCustomAdapter,
  getAttentionReport,
  getCustomAdapterReport,
  getOverviewReport,
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
  OverviewReport,
  ProcessScannerReport,
  StorageReport,
  TraySurfaceReport,
} from "./lib/tauri";
import { Sidebar } from "./components/Sidebar";
import type { PageId, SidebarItem } from "./components/Sidebar";
import { OverviewPage } from "./pages/OverviewPage";
import { AttentionPage } from "./pages/AttentionPage";
import { DiagnosticsPage } from "./pages/DiagnosticsPage";

type AsyncState<T> =
  | { kind: "loading" }
  | { kind: "ready"; report: T }
  | { kind: "error"; message: string };

export function App(): JSX.Element {
  const [page, setPage] = useState<PageId>("overview");

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
  const [attentionBusy, setAttentionBusy] = useState(false);
  const [overview, setOverview] = useState<AsyncState<OverviewReport>>({
    kind: "loading",
  });
  const [ticking, setTicking] = useState(false);
  const [lastTickAt, setLastTickAt] = useState<string | null>(null);

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

  const refreshAttention = useCallback(async (): Promise<void> => {
    try {
      const report = await getAttentionReport();
      setAttention({ kind: "ready", report });
    } catch (err) {
      setAttention({ kind: "error", message: describeError(err) });
    }
  }, []);

  const refreshOverview = useCallback(async (): Promise<void> => {
    try {
      const report = await getOverviewReport();
      setOverview({ kind: "ready", report });
    } catch (err) {
      setOverview({ kind: "error", message: describeError(err) });
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

  const handleRunTick = useCallback(async (): Promise<void> => {
    setTicking(true);
    try {
      const tickReport = await runMonitorTick();
      setLastTickAt(tickReport.tickAt);
      await Promise.all([
        refreshOverview(),
        refreshAttention(),
        refreshScanner(),
      ]);
    } finally {
      setTicking(false);
    }
  }, [refreshAttention, refreshOverview, refreshScanner]);

  const handleMuteAttention = useCallback(
    async (id: string, hours: number): Promise<void> => {
      setAttentionBusy(true);
      try {
        await muteAttentionItem(id, hours);
        await Promise.all([refreshAttention(), refreshOverview()]);
      } catch (err) {
        setAttention({ kind: "error", message: describeError(err) });
      } finally {
        setAttentionBusy(false);
      }
    },
    [refreshAttention, refreshOverview],
  );

  const handleResolveAttention = useCallback(
    async (id: string): Promise<void> => {
      setAttentionBusy(true);
      try {
        await resolveAttentionItem(id);
        await Promise.all([refreshAttention(), refreshOverview()]);
      } catch (err) {
        setAttention({ kind: "error", message: describeError(err) });
      } finally {
        setAttentionBusy(false);
      }
    },
    [refreshAttention, refreshOverview],
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
    void refreshOverview();
    return () => {
      cancelled = true;
    };
  }, [refreshScanner, refreshCustom, refreshAttention, refreshOverview]);

  const navItems: SidebarItem[] = [
    {
      id: "overview",
      label: "Overview",
    },
    {
      id: "attention",
      label: "Attention",
      ...attentionNavBadge(attention),
    },
    {
      id: "diagnostics",
      label: "Diagnostics",
      ...diagnosticsNavBadge(storage, tray),
    },
  ];

  return (
    <div className="app">
      <Sidebar
        items={navItems}
        activeId={page}
        onSelect={setPage}
        refreshing={ticking}
        onRefresh={() => {
          void handleRunTick();
        }}
        lastTickAt={lastTickAt}
      />

      <main className="app__main" role="main">
        {page === "overview" && (
          <OverviewPage
            state={overview}
            busy={attentionBusy}
            onMute={(id, hours) => {
              void handleMuteAttention(id, hours);
            }}
            onResolve={(id) => {
              void handleResolveAttention(id);
            }}
            onOpenAttention={() => setPage("attention")}
          />
        )}
        {page === "attention" && (
          <AttentionPage
            state={attention}
            busy={attentionBusy}
            onMute={(id, hours) => {
              void handleMuteAttention(id, hours);
            }}
            onResolve={(id) => {
              void handleResolveAttention(id);
            }}
          />
        )}
        {page === "diagnostics" && (
          <DiagnosticsPage
            tray={tray}
            storage={storage}
            scanner={scanner}
            rescanning={rescanning}
            onRescan={() => {
              void refreshScanner();
            }}
            custom={custom}
            customBusy={customBusy}
            onAddCustom={handleAddCustom}
            onDeleteCustom={handleDeleteCustom}
            onToggleCustom={handleToggleCustom}
          />
        )}
      </main>
    </div>
  );
}

function attentionNavBadge(
  state: AsyncState<AttentionReport>,
): { badge?: string | null; badgeKind?: "warn" | "bad" | "ok" | "neutral" } {
  if (state.kind !== "ready") return {};
  const count = state.report.items.length;
  if (count === 0) return {};
  const urgent = state.report.items.filter(
    (e) => e.item.severity === "urgent",
  ).length;
  const warn = state.report.items.filter((e) => e.item.severity === "warn")
    .length;
  if (urgent > 0) return { badge: String(count), badgeKind: "bad" };
  if (warn > 0) return { badge: String(count), badgeKind: "warn" };
  return { badge: String(count), badgeKind: "neutral" };
}

function diagnosticsNavBadge(
  storage: AsyncState<StorageReport>,
  tray: AsyncState<TraySurfaceReport>,
): { badge?: string | null; badgeKind?: "warn" | "bad" | "ok" | "neutral" } {
  if (storage.kind === "ready" && !storage.report.ready) {
    return { badge: "!", badgeKind: "bad" };
  }
  if (
    storage.kind === "ready" &&
    storage.report.ready &&
    storage.report.error != null
  ) {
    return { badge: "!", badgeKind: "warn" };
  }
  if (tray.kind === "ready" && tray.report.fallbackRequired) {
    return { badge: "tray", badgeKind: "warn" };
  }
  return {};
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
