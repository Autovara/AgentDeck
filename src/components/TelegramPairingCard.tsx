import { useEffect, useState } from "react";
import type { JSX } from "react";
import type {
  AllowlistEntry,
  PendingPairingPayload,
  TelegramStatusReport,
} from "../lib/tauri";

type AsyncState<T> =
  | { kind: "loading" }
  | { kind: "ready"; report: T }
  | { kind: "error"; message: string };

type Props = {
  state: AsyncState<TelegramStatusReport>;
  busy: boolean;
  onEnable: () => Promise<void> | void;
  onDisable: () => Promise<void> | void;
  onSaveToken: (token: string) => Promise<void> | void;
  onClearToken: () => Promise<void> | void;
  onGenerateCode: () => Promise<void> | void;
  onCancelPairing: () => Promise<void> | void;
  onRevokeUser: (userId: number) => Promise<void> | void;
};

export function TelegramPairingCard({
  state,
  busy,
  onEnable,
  onDisable,
  onSaveToken,
  onClearToken,
  onGenerateCode,
  onCancelPairing,
  onRevokeUser,
}: Props): JSX.Element {
  return (
    <section className="card">
      <header className="card__header">
        <h2>Telegram pairing</h2>
        <StatusPill state={state} />
      </header>

      {state.kind === "loading" && (
        <p className="card__body">Loading Telegram state…</p>
      )}

      {state.kind === "error" && (
        <div className="card__body card__body--error">
          <p>
            <strong>Could not reach the Telegram state.</strong>
          </p>
          <pre className="card__diagnostic">{state.message}</pre>
        </div>
      )}

      {state.kind === "ready" && (
        <TelegramDetails
          report={state.report}
          busy={busy}
          onEnable={onEnable}
          onDisable={onDisable}
          onSaveToken={onSaveToken}
          onClearToken={onClearToken}
          onGenerateCode={onGenerateCode}
          onCancelPairing={onCancelPairing}
          onRevokeUser={onRevokeUser}
        />
      )}
    </section>
  );
}

function StatusPill({
  state,
}: {
  state: AsyncState<TelegramStatusReport>;
}): JSX.Element {
  if (state.kind === "loading") {
    return <span className="pill pill--neutral">Loading</span>;
  }
  if (state.kind === "error") {
    return <span className="pill pill--warn">Unreachable</span>;
  }
  const r = state.report;
  if (!r.ready) return <span className="pill pill--warn">Storage offline</span>;
  if (!r.enabled) return <span className="pill pill--neutral">Disabled</span>;
  if (!r.hasToken) return <span className="pill pill--warn">No token</span>;
  if (!r.running) return <span className="pill pill--warn">Not running</span>;
  if (r.allowlist.length === 0)
    return <span className="pill pill--neutral">Awaiting pairing</span>;
  return (
    <span className="pill pill--ok">
      Paired ({r.allowlist.length})
    </span>
  );
}

function TelegramDetails({
  report,
  busy,
  onEnable,
  onDisable,
  onSaveToken,
  onClearToken,
  onGenerateCode,
  onCancelPairing,
  onRevokeUser,
}: {
  report: TelegramStatusReport;
  busy: boolean;
  onEnable: Props["onEnable"];
  onDisable: Props["onDisable"];
  onSaveToken: Props["onSaveToken"];
  onClearToken: Props["onClearToken"];
  onGenerateCode: Props["onGenerateCode"];
  onCancelPairing: Props["onCancelPairing"];
  onRevokeUser: Props["onRevokeUser"];
}): JSX.Element {
  return (
    <div className="card__body">
      <p className="card__hint">
        AgentDeck stays local-first by default. Pairing a Telegram bot lets you
        check status and (later) trigger safe actions from your phone. The bot
        token is stored locally in your AgentDeck database.{" "}
        <strong>Only the Telegram users you pair</strong> can talk to the bot.
      </p>

      <dl className="kv">
        <Row label="Enabled" value={report.enabled ? "Yes" : "No"} />
        <Row label="Token saved" value={report.hasToken ? "Yes" : "No"} />
        <Row label="Bot running" value={report.running ? "Yes" : "No"} />
        <Row
          label="Paired users"
          value={
            report.allowlist.length === 0
              ? "None"
              : `${report.allowlist.length}`
          }
        />
      </dl>

      {report.error != null && (
        <div className="card__body--error" style={{ marginTop: 12 }}>
          <pre className="card__diagnostic">{report.error}</pre>
        </div>
      )}

      <EnableSection
        report={report}
        busy={busy}
        onEnable={onEnable}
        onDisable={onDisable}
      />

      <TokenSection
        report={report}
        busy={busy}
        onSaveToken={onSaveToken}
        onClearToken={onClearToken}
      />

      <PairingSection
        report={report}
        busy={busy}
        onGenerateCode={onGenerateCode}
        onCancelPairing={onCancelPairing}
      />

      <AllowlistSection
        report={report}
        busy={busy}
        onRevokeUser={onRevokeUser}
      />
    </div>
  );
}

function EnableSection({
  report,
  busy,
  onEnable,
  onDisable,
}: {
  report: TelegramStatusReport;
  busy: boolean;
  onEnable: Props["onEnable"];
  onDisable: Props["onDisable"];
}): JSX.Element {
  return (
    <div className="card__section">
      <h3 className="card__section-title">1. Enable Telegram</h3>
      <p className="card__hint">
        Disabled by default. Enabling does not start the bot until a token is
        saved.
      </p>
      <div className="custom-adapter-form__actions">
        {report.enabled ? (
          <button
            type="button"
            className="card__button"
            disabled={busy}
            onClick={() => {
              void onDisable();
            }}
          >
            Disable Telegram
          </button>
        ) : (
          <button
            type="button"
            className="card__button"
            disabled={busy}
            onClick={() => {
              void onEnable();
            }}
          >
            Enable Telegram
          </button>
        )}
      </div>
    </div>
  );
}

function TokenSection({
  report,
  busy,
  onSaveToken,
  onClearToken,
}: {
  report: TelegramStatusReport;
  busy: boolean;
  onSaveToken: Props["onSaveToken"];
  onClearToken: Props["onClearToken"];
}): JSX.Element {
  const [token, setToken] = useState("");
  const [error, setError] = useState<string | null>(null);

  return (
    <div className="card__section">
      <h3 className="card__section-title">2. Bot token</h3>
      <p className="card__hint">
        Create a bot with{" "}
        <a
          href="https://t.me/BotFather"
          target="_blank"
          rel="noreferrer"
        >
          @BotFather
        </a>{" "}
        and paste the token below. AgentDeck stores it locally; it is never
        sent to any server other than Telegram.
      </p>
      <form
        onSubmit={async (e) => {
          e.preventDefault();
          setError(null);
          const trimmed = token.trim();
          if (trimmed === "") {
            setError("Bot token must not be empty.");
            return;
          }
          try {
            await onSaveToken(trimmed);
            setToken("");
          } catch (err) {
            setError(describeError(err));
          }
        }}
      >
        <label className="custom-adapter-form__pattern">
          <span>Token</span>
          <input
            type="password"
            autoComplete="off"
            spellCheck={false}
            value={token}
            onChange={(e) => setToken(e.target.value)}
            placeholder={
              report.hasToken
                ? "Replace existing token…"
                : "123456:ABC-DEF1234ghIkl-zyx57W2v1u123ew11"
            }
          />
        </label>
        {error != null && (
          <pre className="card__diagnostic" style={{ marginBottom: 8 }}>
            {error}
          </pre>
        )}
        <div className="custom-adapter-form__actions">
          <button
            type="submit"
            className="card__button"
            disabled={busy || token.trim() === ""}
          >
            {busy ? "Saving…" : "Save token"}
          </button>
          {report.hasToken && (
            <button
              type="button"
              className="card__button"
              disabled={busy}
              style={{ marginLeft: 8 }}
              onClick={() => {
                if (
                  confirm(
                    "Clear the saved Telegram bot token? The bot will stop and existing pairings will be kept.",
                  )
                ) {
                  void onClearToken();
                }
              }}
            >
              Clear token
            </button>
          )}
        </div>
      </form>
    </div>
  );
}

function PairingSection({
  report,
  busy,
  onGenerateCode,
  onCancelPairing,
}: {
  report: TelegramStatusReport;
  busy: boolean;
  onGenerateCode: Props["onGenerateCode"];
  onCancelPairing: Props["onCancelPairing"];
}): JSX.Element {
  const canPair = report.enabled && report.hasToken && report.running;
  const pending = report.pendingPairing;

  return (
    <div className="card__section">
      <h3 className="card__section-title">3. Pair a Telegram user</h3>
      {!canPair && (
        <p className="card__hint">
          Enable Telegram and save a bot token first.
        </p>
      )}
      {canPair && pending == null && (
        <>
          <p className="card__hint">
            Generate a single-use code, then DM it to your bot as{" "}
            <code>PAIR &lt;code&gt;</code> within 10 minutes.
          </p>
          <div className="custom-adapter-form__actions">
            <button
              type="button"
              className="card__button"
              disabled={busy}
              onClick={() => {
                void onGenerateCode();
              }}
            >
              Generate pairing code
            </button>
          </div>
        </>
      )}
      {canPair && pending != null && (
        <PendingPairingDisplay
          pending={pending}
          busy={busy}
          onCancelPairing={onCancelPairing}
        />
      )}
    </div>
  );
}

function PendingPairingDisplay({
  pending,
  busy,
  onCancelPairing,
}: {
  pending: PendingPairingPayload;
  busy: boolean;
  onCancelPairing: Props["onCancelPairing"];
}): JSX.Element {
  const remaining = usePairingCountdown(pending.expiresAt);

  return (
    <div>
      <p className="card__hint">
        DM the following to your bot within{" "}
        <strong>{remaining}</strong>:
      </p>
      <pre
        className="card__diagnostic"
        style={{ fontSize: "1.25rem", textAlign: "center", padding: 16 }}
      >
        PAIR {pending.code}
      </pre>
      <div className="custom-adapter-form__actions">
        <button
          type="button"
          className="card__button"
          disabled={busy}
          onClick={() => {
            void navigator.clipboard?.writeText(`PAIR ${pending.code}`);
          }}
        >
          Copy
        </button>
        <button
          type="button"
          className="card__button"
          disabled={busy}
          style={{ marginLeft: 8 }}
          onClick={() => {
            void onCancelPairing();
          }}
        >
          Cancel pairing
        </button>
      </div>
    </div>
  );
}

function AllowlistSection({
  report,
  busy,
  onRevokeUser,
}: {
  report: TelegramStatusReport;
  busy: boolean;
  onRevokeUser: Props["onRevokeUser"];
}): JSX.Element {
  return (
    <div className="card__section">
      <h3 className="card__section-title">4. Paired users</h3>
      {report.allowlist.length === 0 ? (
        <p className="card__hint">No Telegram users paired yet.</p>
      ) : (
        <table className="table">
          <thead>
            <tr>
              <th>User</th>
              <th>Paired</th>
              <th style={{ textAlign: "right" }}>Action</th>
            </tr>
          </thead>
          <tbody>
            {report.allowlist.map((entry) => (
              <AllowlistRow
                key={entry.userId}
                entry={entry}
                busy={busy}
                onRevokeUser={onRevokeUser}
              />
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}

function AllowlistRow({
  entry,
  busy,
  onRevokeUser,
}: {
  entry: AllowlistEntry;
  busy: boolean;
  onRevokeUser: Props["onRevokeUser"];
}): JSX.Element {
  const label = entry.username != null ? `@${entry.username}` : `id ${entry.userId}`;
  return (
    <tr>
      <td title={`Telegram user id ${entry.userId}`}>{label}</td>
      <td>{formatTimestamp(entry.pairedAt)}</td>
      <td style={{ textAlign: "right" }}>
        <button
          type="button"
          className="card__button"
          disabled={busy}
          onClick={() => {
            if (
              confirm(
                `Revoke ${label}? They will no longer be able to talk to the bot.`,
              )
            ) {
              void onRevokeUser(entry.userId);
            }
          }}
        >
          Revoke
        </button>
      </td>
    </tr>
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

function usePairingCountdown(expiresAt: string): string {
  const [, force] = useState(0);
  useEffect(() => {
    const t = setInterval(() => force((n) => n + 1), 1000);
    return () => clearInterval(t);
  }, []);
  const diff = new Date(expiresAt).getTime() - Date.now();
  if (diff <= 0) return "expired";
  const totalSeconds = Math.floor(diff / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${minutes}:${seconds.toString().padStart(2, "0")}`;
}

function formatTimestamp(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleString();
}

function describeError(err: unknown): string {
  if (err instanceof Error) return err.message;
  if (typeof err === "string") return err;
  return JSON.stringify(err);
}
