import type { JSX, ReactNode } from "react";

type Props = {
  label: string;
  value: ReactNode;
  /** Optional helper text rendered beneath the value. */
  caption?: ReactNode;
  /** Optional accent ("ok" / "warn" / "bad" / "neutral"). */
  kind?: "ok" | "warn" | "bad" | "neutral";
};

export function SummaryTile({
  label,
  value,
  caption,
  kind = "neutral",
}: Props): JSX.Element {
  return (
    <div className={"summary-tile summary-tile--" + kind}>
      <div className="summary-tile__label">{label}</div>
      <div className="summary-tile__value">{value}</div>
      {caption != null && (
        <div className="summary-tile__caption">{caption}</div>
      )}
    </div>
  );
}
