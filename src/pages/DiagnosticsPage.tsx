import type { JSX } from "react";
import { CustomAdaptersCard } from "../components/CustomAdaptersCard";
import { ProcessScannerCard } from "../components/ProcessScannerCard";
import { StorageCard } from "../components/StorageCard";
import { TraySurfaceCard } from "../components/TraySurfaceCard";
import type {
  CustomAdapterReport,
  NewCustomAdapter,
  ProcessScannerReport,
  StorageReport,
  TraySurfaceReport,
} from "../lib/tauri";

type AsyncState<T> =
  | { kind: "loading" }
  | { kind: "ready"; report: T }
  | { kind: "error"; message: string };

type Props = {
  tray: AsyncState<TraySurfaceReport>;
  storage: AsyncState<StorageReport>;
  scanner: AsyncState<ProcessScannerReport>;
  rescanning: boolean;
  onRescan: () => void;
  custom: AsyncState<CustomAdapterReport>;
  customBusy: boolean;
  onAddCustom: (input: NewCustomAdapter) => Promise<void>;
  onDeleteCustom: (id: string) => Promise<void>;
  onToggleCustom: (id: string, enabled: boolean) => Promise<void>;
};

export function DiagnosticsPage({
  tray,
  storage,
  scanner,
  rescanning,
  onRescan,
  custom,
  customBusy,
  onAddCustom,
  onDeleteCustom,
  onToggleCustom,
}: Props): JSX.Element {
  return (
    <section className="page page--diagnostics">
      <header className="page__header">
        <div>
          <h1 className="page__title">Diagnostics</h1>
          <p className="page__subtitle">
            Live state of the host environment, the local database, the
            process scanner, and the user-defined custom adapters. The
            adapter-diagnostics table itself lands in build-plan §15 step 9.
          </p>
        </div>
      </header>

      <div className="page__body">
        <TraySurfaceCard state={tray} />
        <StorageCard state={storage} />
        <ProcessScannerCard
          state={scanner}
          onRescan={onRescan}
          rescanning={rescanning}
        />
        <CustomAdaptersCard
          state={custom}
          busy={customBusy}
          onAdd={onAddCustom}
          onDelete={onDeleteCustom}
          onToggle={onToggleCustom}
        />
      </div>
    </section>
  );
}
