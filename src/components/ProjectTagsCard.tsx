import { useState } from "react";
import type { JSX } from "react";
import type { NewProjectTag, ProjectTag, ProjectTagsReport } from "../lib/tauri";

type AsyncState<T> =
  | { kind: "loading" }
  | { kind: "ready"; report: T }
  | { kind: "error"; message: string };

type Props = {
  state: AsyncState<ProjectTagsReport>;
  busy: boolean;
  onCreate: (input: NewProjectTag) => Promise<void> | void;
  onDelete: (id: string) => Promise<void> | void;
};

export function ProjectTagsCard({
  state,
  busy,
  onCreate,
  onDelete,
}: Props): JSX.Element {
  return (
    <section className="card">
      <header className="card__header">
        <h2>Project tags</h2>
        <StatusPill state={state} />
      </header>

      {state.kind === "loading" && (
        <p className="card__body">Loading tags…</p>
      )}

      {state.kind === "error" && (
        <div className="card__body card__body--error">
          <pre className="card__diagnostic">{state.message}</pre>
        </div>
      )}

      {state.kind === "ready" && (
        <Details
          report={state.report}
          busy={busy}
          onCreate={onCreate}
          onDelete={onDelete}
        />
      )}
    </section>
  );
}

function StatusPill({
  state,
}: {
  state: AsyncState<ProjectTagsReport>;
}): JSX.Element {
  if (state.kind === "loading")
    return <span className="pill pill--neutral">Loading</span>;
  if (state.kind === "error")
    return <span className="pill pill--warn">Unreachable</span>;
  if (!state.report.ready)
    return <span className="pill pill--warn">Offline</span>;
  return (
    <span className="pill pill--ok">{state.report.tags.length} tags</span>
  );
}

function Details({
  report,
  busy,
  onCreate,
  onDelete,
}: {
  report: ProjectTagsReport;
  busy: boolean;
  onCreate: Props["onCreate"];
  onDelete: Props["onDelete"];
}): JSX.Element {
  return (
    <div className="card__body">
      <p className="card__hint">
        Group sessions by project or client. Tags are referenced by name in
        each session row — renaming a tag is not yet supported (delete + create
        a new one).
      </p>

      {report.error != null && (
        <div className="card__body--error" style={{ marginTop: 12 }}>
          <pre className="card__diagnostic">{report.error}</pre>
        </div>
      )}

      {report.tags.length === 0 ? (
        <p className="card__hint">No tags defined yet.</p>
      ) : (
        <table className="table">
          <thead>
            <tr>
              <th>Name</th>
              <th>Color</th>
              <th>Notes</th>
              <th style={{ textAlign: "right" }}>Action</th>
            </tr>
          </thead>
          <tbody>
            {report.tags.map((t) => (
              <TagRow key={t.id} tag={t} busy={busy} onDelete={onDelete} />
            ))}
          </tbody>
        </table>
      )}

      <details className="card__details">
        <summary>Add tag</summary>
        <AddForm onCreate={onCreate} busy={busy} />
      </details>
    </div>
  );
}

function TagRow({
  tag,
  busy,
  onDelete,
}: {
  tag: ProjectTag;
  busy: boolean;
  onDelete: Props["onDelete"];
}): JSX.Element {
  return (
    <tr>
      <td>
        {tag.color != null && (
          <span
            aria-hidden
            style={{
              display: "inline-block",
              width: 8,
              height: 8,
              borderRadius: "50%",
              background: tag.color,
              marginRight: 6,
              verticalAlign: "middle",
            }}
          />
        )}
        {tag.name}
      </td>
      <td>{tag.color ?? "—"}</td>
      <td title={tag.notes ?? undefined}>
        {tag.notes != null ? truncate(tag.notes, 40) : "—"}
      </td>
      <td style={{ textAlign: "right" }}>
        <button
          type="button"
          className="card__button"
          disabled={busy}
          onClick={() => {
            if (
              confirm(
                `Delete tag "${tag.name}"? Any session currently tagged will lose its tag.`,
              )
            ) {
              void onDelete(tag.id);
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
  onCreate,
  busy,
}: {
  onCreate: Props["onCreate"];
  busy: boolean;
}): JSX.Element {
  const [name, setName] = useState("");
  const [color, setColor] = useState("");
  const [notes, setNotes] = useState("");
  const [error, setError] = useState<string | null>(null);

  return (
    <form
      className="custom-adapter-form"
      onSubmit={async (e) => {
        e.preventDefault();
        setError(null);
        try {
          await onCreate({
            name: name.trim(),
            color: color.trim() === "" ? null : color.trim(),
            notes: notes.trim() === "" ? null : notes.trim(),
          });
          setName("");
          setColor("");
          setNotes("");
        } catch (err) {
          setError(describeError(err));
        }
      }}
    >
      <div className="custom-adapter-form__row">
        <label>
          <span>Name</span>
          <input
            type="text"
            required
            maxLength={64}
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="e.g. billing-api"
          />
        </label>
        <label>
          <span>Color</span>
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
        <span>Notes</span>
        <input
          type="text"
          value={notes}
          onChange={(e) => setNotes(e.target.value)}
          placeholder="optional"
          maxLength={256}
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
          disabled={busy || name.trim() === ""}
        >
          {busy ? "Saving…" : "Add tag"}
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
