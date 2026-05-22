import { useState } from "react";
import type { JSX } from "react";
import type {
  CustomAdapterMatchKind,
  CustomAdapterReport,
  CustomAdapterSummary,
  NewCustomAdapter,
} from "../lib/tauri";

type Props = {
  state:
    | { kind: "loading" }
    | { kind: "ready"; report: CustomAdapterReport }
    | { kind: "error"; message: string };
  onAdd: (input: NewCustomAdapter) => Promise<void>;
  onDelete: (id: string) => Promise<void>;
  onToggle: (id: string, enabled: boolean) => Promise<void>;
  /** True while any mutation is in flight. */
  busy: boolean;
};

const MATCH_KINDS: CustomAdapterMatchKind[] = ["name", "cmdline", "cwd"];

export function CustomAdaptersCard({
  state,
  onAdd,
  onDelete,
  onToggle,
  busy,
}: Props): JSX.Element {
  return (
    <section className="card">
      <header className="card__header">
        <h2>Custom adapters</h2>
        <div className="card__header-actions">
          <StatusPill state={state} />
        </div>
      </header>

      {state.kind === "loading" && (
        <p className="card__body">Loading custom adapters…</p>
      )}

      {state.kind === "error" && (
        <div className="card__body card__body--error">
          <p>
            <strong>Custom adapter registry unreachable.</strong>
          </p>
          <pre className="card__diagnostic">{state.message}</pre>
        </div>
      )}

      {state.kind === "ready" && (
        <CustomAdapterDetails
          report={state.report}
          busy={busy}
          onAdd={onAdd}
          onDelete={onDelete}
          onToggle={onToggle}
        />
      )}
    </section>
  );
}

function StatusPill({ state }: Pick<Props, "state">): JSX.Element {
  if (state.kind === "loading") {
    return <span className="pill pill--neutral">Probing</span>;
  }
  if (state.kind === "error") {
    return <span className="pill pill--warn">Unreachable</span>;
  }
  if (!state.report.ready) {
    return <span className="pill pill--warn">Offline</span>;
  }
  if (state.report.error != null) {
    return <span className="pill pill--warn">Errored</span>;
  }
  return (
    <span className="pill pill--ok">
      {state.report.adapters.length} defined
    </span>
  );
}

function CustomAdapterDetails({
  report,
  busy,
  onAdd,
  onDelete,
  onToggle,
}: {
  report: CustomAdapterReport;
  busy: boolean;
  onAdd: Props["onAdd"];
  onDelete: Props["onDelete"];
  onToggle: Props["onToggle"];
}): JSX.Element {
  return (
    <div className="card__body">
      <p className="card__hint">
        Define a regex matcher (name, command line, or cwd) and AgentDeck will
        treat any matching process as an agent for tracking. Custom adapters
        are locked to capability level 1 (presence detection only).
      </p>

      {report.error != null && (
        <div className="card__body--error" style={{ marginTop: 12 }}>
          <pre className="card__diagnostic">{report.error}</pre>
        </div>
      )}

      {report.adapters.length === 0 ? (
        <p className="card__hint">No custom adapters defined yet.</p>
      ) : (
        <table className="table">
          <thead>
            <tr>
              <th>Label</th>
              <th>Match</th>
              <th>Pattern</th>
              <th>Matched PIDs</th>
              <th style={{ textAlign: "right" }}>Actions</th>
            </tr>
          </thead>
          <tbody>
            {report.adapters.map((a) => (
              <AdapterRow
                key={a.id}
                adapter={a}
                busy={busy}
                onDelete={onDelete}
                onToggle={onToggle}
              />
            ))}
          </tbody>
        </table>
      )}

      <details className="card__details">
        <summary>Add custom adapter</summary>
        <AddForm onAdd={onAdd} busy={busy} />
      </details>
    </div>
  );
}

function AdapterRow({
  adapter,
  busy,
  onDelete,
  onToggle,
}: {
  adapter: CustomAdapterSummary;
  busy: boolean;
  onDelete: Props["onDelete"];
  onToggle: Props["onToggle"];
}): JSX.Element {
  const matchSummary = `${adapter.matchKind}${adapter.enabled ? "" : " (off)"}`;
  return (
    <tr style={{ opacity: adapter.enabled ? 1 : 0.55 }}>
      <td title={adapter.notes ?? undefined}>
        {adapter.color != null && (
          <span
            aria-hidden
            style={{
              display: "inline-block",
              width: 8,
              height: 8,
              borderRadius: "50%",
              background: adapter.color,
              marginRight: 6,
              verticalAlign: "middle",
            }}
          />
        )}
        {adapter.label}
        {adapter.regexCompileError != null && (
          <span className="pill pill--warn" style={{ marginLeft: 6 }}>
            regex error
          </span>
        )}
      </td>
      <td>{matchSummary}</td>
      <td title={adapter.pattern}>{truncate(adapter.pattern, 32)}</td>
      <td>
        {adapter.matchedPids.length === 0
          ? "—"
          : adapter.matchedPids.slice(0, 6).join(", ") +
            (adapter.matchedPids.length > 6
              ? ` +${adapter.matchedPids.length - 6}`
              : "")}
      </td>
      <td style={{ textAlign: "right" }}>
        <button
          type="button"
          className="card__button"
          disabled={busy}
          onClick={() => void onToggle(adapter.id, !adapter.enabled)}
        >
          {adapter.enabled ? "Disable" : "Enable"}
        </button>
        <button
          type="button"
          className="card__button"
          disabled={busy}
          style={{ marginLeft: 4 }}
          onClick={() => {
            if (confirm(`Delete custom adapter "${adapter.label}"?`)) {
              void onDelete(adapter.id);
            }
          }}
        >
          Delete
        </button>
      </td>
    </tr>
  );
}

function AddForm({
  onAdd,
  busy,
}: {
  onAdd: Props["onAdd"];
  busy: boolean;
}): JSX.Element {
  const [label, setLabel] = useState("");
  const [agentName, setAgentName] = useState("");
  const [matchKind, setMatchKind] =
    useState<CustomAdapterMatchKind>("cmdline");
  const [pattern, setPattern] = useState("");
  const [color, setColor] = useState("");
  const [error, setError] = useState<string | null>(null);

  return (
    <form
      className="custom-adapter-form"
      onSubmit={async (e) => {
        e.preventDefault();
        setError(null);
        try {
          await onAdd({
            label: label.trim(),
            agentName: (agentName.trim() || label.trim()).slice(0, 64),
            matchKind,
            pattern: pattern.trim(),
            color: color.trim() === "" ? null : color.trim(),
          });
          setLabel("");
          setAgentName("");
          setPattern("");
          setColor("");
        } catch (err) {
          setError(describeError(err));
        }
      }}
    >
      <div className="custom-adapter-form__row">
        <label>
          <span>Label</span>
          <input
            type="text"
            required
            maxLength={64}
            value={label}
            onChange={(e) => setLabel(e.target.value)}
            placeholder="e.g. aider-dev"
          />
        </label>
        <label>
          <span>Agent name</span>
          <input
            type="text"
            maxLength={64}
            value={agentName}
            onChange={(e) => setAgentName(e.target.value)}
            placeholder="defaults to label"
          />
        </label>
      </div>
      <div className="custom-adapter-form__row">
        <label>
          <span>Match against</span>
          <select
            value={matchKind}
            onChange={(e) =>
              setMatchKind(e.target.value as CustomAdapterMatchKind)
            }
          >
            {MATCH_KINDS.map((k) => (
              <option key={k} value={k}>
                {k}
              </option>
            ))}
          </select>
        </label>
        <label>
          <span>Colour</span>
          <input
            type="text"
            value={color}
            onChange={(e) => setColor(e.target.value)}
            placeholder="#4c6ef5"
            maxLength={7}
          />
        </label>
      </div>
      <label className="custom-adapter-form__pattern">
        <span>Pattern (Rust regex)</span>
        <input
          type="text"
          required
          maxLength={1024}
          value={pattern}
          onChange={(e) => setPattern(e.target.value)}
          placeholder="aider|claude-code"
        />
      </label>
      {error != null && (
        <pre className="card__diagnostic" style={{ marginBottom: 8 }}>
          {error}
        </pre>
      )}
      <div className="custom-adapter-form__actions">
        <button type="submit" className="card__button" disabled={busy}>
          {busy ? "Saving…" : "Add adapter"}
        </button>
      </div>
    </form>
  );
}

function truncate(s: string, max: number): string {
  if (s.length <= max) return s;
  return `${s.slice(0, max - 1)}…`;
}

function describeError(err: unknown): string {
  if (err instanceof Error) return err.message;
  if (typeof err === "string") return err;
  return JSON.stringify(err);
}
