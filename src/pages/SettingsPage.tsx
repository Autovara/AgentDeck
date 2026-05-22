import type { JSX } from "react";
import type {
  NewProjectTag,
  ProjectTagsReport,
  TelegramStatusReport,
} from "../lib/tauri";
import { TelegramPairingCard } from "../components/TelegramPairingCard";
import { ProjectTagsCard } from "../components/ProjectTagsCard";
import { ExportCard } from "../components/ExportCard";

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

  tags: AsyncState<ProjectTagsReport>;
  tagsBusy: boolean;
  onCreateTag: (input: NewProjectTag) => Promise<void> | void;
  onDeleteTag: (id: string) => Promise<void> | void;

  storageReady: boolean;
};

export function SettingsPage(props: Props): JSX.Element {
  return (
    <div className="page">
      <header className="page__header">
        <h1>Settings</h1>
        <p className="page__lead">
          Local preferences and integrations. Project tags, export, and
          Telegram pairing live here; pricing / scan interval / budget alerts
          land in later alpha builds.
        </p>
      </header>

      <div className="page__grid">
        <ProjectTagsCard
          state={props.tags}
          busy={props.tagsBusy}
          onCreate={props.onCreateTag}
          onDelete={props.onDeleteTag}
        />
        <ExportCard storageReady={props.storageReady} />
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
