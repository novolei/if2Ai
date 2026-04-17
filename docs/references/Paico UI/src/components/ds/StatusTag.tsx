// ─── Types ────────────────────────────────────────────────────────────────────

export type StatusVariant =
  | "active"
  | "pending"
  | "warning"
  | "success"
  | "error"
  | "neutral";

export interface StatusTagProps {
  /** Semantic status */
  status?: StatusVariant;
  /** Label text (defaults to localised status name) */
  label?: string;
  /** Show dot indicator before label */
  dot?: boolean;
}

// ─── Labels ───────────────────────────────────────────────────────────────────

const DEFAULT_LABELS: Record<StatusVariant, string> = {
  active:  "进行中",
  pending: "等待中",
  warning: "警告",
  success: "成功",
  error:   "错误",
  neutral: "中性",
};

// ─── Component ────────────────────────────────────────────────────────────────
/*
 * Structure
 * ┌──────────────────────────────────────┐
 * │  [●]  标签文字                        │
 * └──────────────────────────────────────┘
 *
 * Sizing & shape
 * • Uses .tag-pill utility from index.css
 * • px-2 py-0.5, rounded-full, text-[11px], font-medium
 *
 * Color tokens used
 * • bg: var(--status-{variant}-bg)
 * • fg: var(--status-{variant})
 *
 * Dot
 * • w-1.5 h-1.5 rounded-full, same fg color as text
 * • only rendered when dot=true
 *
 * No external colors — all driven by CSS variables.
 */
export default function StatusTag({
  status = "active",
  label,
  dot    = false,
}: StatusTagProps) {
  const text = label ?? DEFAULT_LABELS[status];

  return (
    <span
      data-cmp="StatusTag"
      data-status={status}
      className="tag-pill inline-flex items-center gap-1"
      style={{
        background: `var(--status-${status}-bg)`,
        color: `var(--status-${status})`,
      }}
    >
      {dot && (
        <span
          className="w-1.5 h-1.5 rounded-full shrink-0"
          style={{ background: `var(--status-${status})` }}
        />
      )}
      {text}
    </span>
  );
}
