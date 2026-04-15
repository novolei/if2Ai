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
/**
 * StatusTag — semantic status badge driven by CSS variables.
 *
 * Uses the `.tag-pill` utility and `--status-{variant}` / `--status-{variant}-bg`
 * tokens from the Paico design system. All colors come from CSS variables,
 * no hardcoded values.
 */
export default function StatusTag({
  status = "active",
  label,
  dot    = false,
}: StatusTagProps) {
  const text = label ?? DEFAULT_LABELS[status];

  return (
    <span
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
