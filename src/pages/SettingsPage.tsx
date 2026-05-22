import type { JSX } from "react";
import type { TelegramStatusReport } from "../lib/tauri";
import { TelegramPairingCard } from "../components/TelegramPairingCard";

type AsyncState<T> =
  | { kind: "loading" }
  | { kind: "ready"; report: T }
  | { kind: "error"; message: string };

type Props = {
  telegram: AsyncState<TelegramStatusReport>;
  telegramBusy: boolean;
  onEnable: () => Promise<void> | void;
  onDisable: () => Promise<void> | void;
  onSaveToken: (token: string) => Promise<void> | void;
  onClearToken: () => Promise<void> | void;
  onGenerateCode: () => Promise<void> | void;
  onCancelPairing: () => Promise<void> | void;
  onRevokeUser: (userId: number) => Promise<void> | void;
};

export function SettingsPage(props: Props): JSX.Element {
  return (
    <div className="page">
      <header className="page__header">
        <h1>Settings</h1>
        <p className="page__lead">
          Local preferences and integrations. Cost / pricing / project tag
          controls land in a later alpha build.
        </p>
      </header>

      <div className="page__grid">
        <TelegramPairingCard
          state={props.telegram}
          busy={props.telegramBusy}
          onEnable={props.onEnable}
          onDisable={props.onDisable}
          onSaveToken={props.onSaveToken}
          onClearToken={props.onClearToken}
          onGenerateCode={props.onGenerateCode}
          onCancelPairing={props.onCancelPairing}
          onRevokeUser={props.onRevokeUser}
        />
      </div>
    </div>
  );
}
