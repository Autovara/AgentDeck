import { useCallback, useEffect, useState } from "react";
import type { JSX } from "react";
import {
  addCustomAdapter,
  assignSessionTag,
  cancelTelegramPairing,
  clearSessionTagAssignment,
  clearTelegramToken,
  createProjectTag,
  deleteCustomAdapter,
  deleteProjectTag,
  disableTelegram,
  enableTelegram,
  generateTelegramPairingCode,
  getAdapterDiagnostics,
  getAttentionReport,
  getCustomAdapterReport,
  getOverviewReport,
  getProcessScannerReport,
  getStorageReport,
  getTelegramStatus,
  getTraySurface,
  listProjectTags,
  listSessions,
  muteAttentionItem,
  resolveAttentionItem,
  revokeTelegramUser,
  runMonitorTick,
  setCustomAdapterEnabled,
  setTelegramToken,
} from "./lib/tauri";
import type {
  AdapterDiagnosticsReport,
  AttentionReport,
  CustomAdapterReport,
  NewCustomAdapter,
  NewProjectTag,
  OverviewReport,
  ProcessScannerReport,
  ProjectTagsReport,
  SessionsReport,
  StorageReport,
  TelegramStatusReport,
  TraySurfaceReport,
} from "./lib/tauri";
import { Sidebar } from "./components/Sidebar";
import type { PageId, SidebarItem } from "./components/Sidebar";
import { OverviewPage } from "./pages/OverviewPage";
import { AttentionPage } from "./pages/AttentionPage";
import { DiagnosticsPage } from "./pages/DiagnosticsPage";
import { SessionsPage } from "./pages/SessionsPage";
import { SettingsPage } from "./pages/SettingsPage";

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
  const [adapterDiagnostics, setAdapterDiagnostics] = useState<
    AsyncState<AdapterDiagnosticsReport>
  >({ kind: "loading" });
  const [telegram, setTelegram] = useState<AsyncState<TelegramStatusReport>>({
    kind: "loading",
  });
  const [telegramBusy, setTelegramBusy] = useState(false);
  const [sessions, setSessions] = useState<AsyncState<SessionsReport>>({
    kind: "loading",
  });
  const [projectTags, setProjectTags] = useState<AsyncState<ProjectTagsReport>>({
    kind: "loading",
  });
  const [tagsBusy, setTagsBusy] = useState(false);
  const [busyTagSession, setBusyTagSession] = useState<string | null>(null);
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

  const refreshAdapterDiagnostics = useCallback(async (): Promise<void> => {
    try {
      const report = await getAdapterDiagnostics();
      setAdapterDiagnostics({ kind: "ready", report });
    } catch (err) {
      setAdapterDiagnostics({ kind: "error", message: describeError(err) });
    }
  }, []);

  const refreshTelegram = useCallback(async (): Promise<void> => {
    try {
      const report = await getTelegramStatus();
      setTelegram({ kind: "ready", report });
    } catch (err) {
      setTelegram({ kind: "error", message: describeError(err) });
    }
  }, []);

  const refreshSessions = useCallback(async (): Promise<void> => {
    try {
      const report = await listSessions();
      setSessions({ kind: "ready", report });
    } catch (err) {
      setSessions({ kind: "error", message: describeError(err) });
    }
  }, []);

  const refreshTags = useCallback(async (): Promise<void> => {
    try {
      const report = await listProjectTags();
      setProjectTags({ kind: "ready", report });
    } catch (err) {
      setProjectTags({ kind: "error", message: describeError(err) });
    }
  }, []);

  const handleCreateTag = useCallback(
    async (input: NewProjectTag): Promise<void> => {
      setTagsBusy(true);
      try {
        const report = await createProjectTag(input);
        setProjectTags({ kind: "ready", report });
      } finally {
        setTagsBusy(false);
      }
    },
    [],
  );

  const handleDeleteTag = useCallback(
    async (id: string): Promise<void> => {
      setTagsBusy(true);
      try {
        const report = await deleteProjectTag(id);
        setProjectTags({ kind: "ready", report });
        // Tag deletion clears it from sessions; refresh so the
        // Sessions page reflects that.
        await refreshSessions();
      } catch (err) {
        setProjectTags({ kind: "error", message: describeError(err) });
      } finally {
        setTagsBusy(false);
      }
    },
    [refreshSessions],
  );

  const handleAssignTag = useCallback(
    async (sessionId: string, tagName: string): Promise<void> => {
      setBusyTagSession(sessionId);
      try {
        await assignSessionTag(sessionId, tagName);
        await refreshSessions();
      } catch (err) {
        setSessions({ kind: "error", message: describeError(err) });
      } finally {
        setBusyTagSession(null);
      }
    },
    [refreshSessions],
  );

  const handleClearTag = useCallback(
    async (sessionId: string): Promise<void> => {
      setBusyTagSession(sessionId);
      try {
        await clearSessionTagAssignment(sessionId);
        await refreshSessions();
      } catch (err) {
        setSessions({ kind: "error", message: describeError(err) });
      } finally {
        setBusyTagSession(null);
      }
    },
    [refreshSessions],
  );

  const runTelegramAction = useCallback(
    async (
      action: () => Promise<TelegramStatusReport | { status: TelegramStatusReport }>,
    ): Promise<void> => {
      setTelegramBusy(true);
      try {
        const result = await action();
        const report =
          "status" in result ? result.status : (result as TelegramStatusReport);
        setTelegram({ kind: "ready", report });
      } catch (err) {
        setTelegram({ kind: "error", message: describeError(err) });
      } finally {
        setTelegramBusy(false);
      }
    },
    [],
  );

  const handleEnableTelegram = useCallback(
    () => runTelegramAction(() => enableTelegram()),
    [runTelegramAction],
  );
  const handleDisableTelegram = useCallback(
    () => runTelegramAction(() => disableTelegram()),
    [runTelegramAction],
  );
  const handleSaveTelegramToken = useCallback(
    (token: string) => runTelegramAction(() => setTelegramToken(token)),
    [runTelegramAction],
  );
  const handleClearTelegramToken = useCallback(
    () => runTelegramAction(() => clearTelegramToken()),
    [runTelegramAction],
  );
  const handleGenerateTelegramCode = useCallback(
    () => runTelegramAction(() => generateTelegramPairingCode()),
    [runTelegramAction],
  );
  const handleCancelTelegramPairing = useCallback(
    () => runTelegramAction(() => cancelTelegramPairing()),
    [runTelegramAction],
  );
  const handleRevokeTelegramUser = useCallback(
    (userId: number) =>
      runTelegramAction(() => revokeTelegramUser(userId)),
    [runTelegramAction],
  );

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
        refreshAdapterDiagnostics(),
        refreshSessions(),
      ]);
    } finally {
      setTicking(false);
    }
  }, [
    refreshAdapterDiagnostics,
    refreshAttention,
    refreshOverview,
    refreshScanner,
    refreshSessions,
  ]);

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
    void refreshAdapterDiagnostics();
    void refreshTelegram();
    void refreshSessions();
    void refreshTags();
    return () => {
      cancelled = true;
    };
  }, [
    refreshScanner,
    refreshCustom,
    refreshAttention,
    refreshOverview,
    refreshAdapterDiagnostics,
    refreshTelegram,
    refreshSessions,
    refreshTags,
  ]);

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
      id: "sessions",
      label: "Sessions",
    },
    {
      id: "diagnostics",
      label: "Diagnostics",
      ...diagnosticsNavBadge(storage, tray, adapterDiagnostics),
    },
    {
      id: "settings",
      label: "Settings",
      ...settingsNavBadge(telegram),
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
            adapterDiagnostics={adapterDiagnostics}
            custom={custom}
            customBusy={customBusy}
            onAddCustom={handleAddCustom}
            onDeleteCustom={handleDeleteCustom}
            onToggleCustom={handleToggleCustom}
          />
        )}
        {page === "sessions" && (
          <SessionsPage
            sessions={sessions}
            tags={projectTags}
            busyTag={busyTagSession}
            onAssignTag={handleAssignTag}
            onClearTag={handleClearTag}
          />
        )}
        {page === "settings" && (
          <SettingsPage
            telegram={telegram}
            telegramBusy={telegramBusy}
            onEnable={handleEnableTelegram}
            onDisable={handleDisableTelegram}
            onSaveToken={handleSaveTelegramToken}
            onClearToken={handleClearTelegramToken}
            onGenerateCode={handleGenerateTelegramCode}
            onCancelPairing={handleCancelTelegramPairing}
            onRevokeUser={handleRevokeTelegramUser}
            tags={projectTags}
            tagsBusy={tagsBusy}
            onCreateTag={handleCreateTag}
            onDeleteTag={handleDeleteTag}
            storageReady={storage.kind === "ready" && storage.report.ready}
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
  adapters: AsyncState<AdapterDiagnosticsReport>,
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
  if (adapters.kind === "ready" && adapters.report.ready) {
    const failing = adapters.report.items.filter(
      (i) => i.failureReasons.length > 0,
    ).length;
    if (failing > 0) {
      return { badge: String(failing), badgeKind: "warn" };
    }
  }
  if (tray.kind === "ready" && tray.report.fallbackRequired) {
    return { badge: "tray", badgeKind: "warn" };
  }
  return {};
}

function settingsNavBadge(
  telegram: AsyncState<TelegramStatusReport>,
): { badge?: string | null; badgeKind?: "warn" | "bad" | "ok" | "neutral" } {
  if (telegram.kind !== "ready") return {};
  const r = telegram.report;
  if (r.enabled && r.hasToken && !r.running) {
    return { badge: "!", badgeKind: "warn" };
  }
  if (r.pendingPairing != null) {
    return { badge: "pair", badgeKind: "warn" };
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
