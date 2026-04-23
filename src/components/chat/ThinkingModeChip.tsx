import { Brain, Sparkles, Zap } from "lucide-react";

import { cn } from "@/lib/utils";

interface ThinkingModeChipProps {
  /** Subset of `Model` capability fields surfaced from the backend
   * `provider::capabilities::resolve()`.  Pass through directly from the
   * `AvailableModel` row in `provider_list_models`. */
  model: {
    reasoning?: boolean;
    reasoning_required_in_tool_calls?: boolean;
    supports_reasoning_effort?: boolean;
  };
  className?: string;
  /** Render as compact (single icon + tooltip only). Default false. */
  compact?: boolean;
}

/**
 * P-MULTI-API — chip rendered next to a model name in the Settings /
 * Onboarding model lists.  Three visual states:
 *
 * - `required` (rose): `reasoning_required_in_tool_calls` (Kimi-thinking,
 *   DeepSeek-R1).  Tooltip explains old session compatibility.
 * - `optional` (jade): `reasoning` true but no required quirk.
 * - `effort` (sky): supports `reasoning_effort` knob (o1 / o3 / GPT-5).
 * - hidden: model has no reasoning capability.
 */
export function ThinkingModeChip({
  model,
  className,
  compact = false,
}: ThinkingModeChipProps) {
  if (!model.reasoning) return null;

  if (model.reasoning_required_in_tool_calls) {
    return (
      <Chip
        tone="rose"
        icon={<Brain className="h-3 w-3" strokeWidth={2} />}
        label={compact ? null : "推理 · 强约束"}
        tooltip="历史 assistant 工具调用必须携带 reasoning_content（Kimi-thinking / DeepSeek-R1）。老会话首条工具调用会自动补空字符串过 400。"
        className={className}
      />
    );
  }
  if (model.supports_reasoning_effort) {
    return (
      <Chip
        tone="sky"
        icon={<Zap className="h-3 w-3" strokeWidth={2} />}
        label={compact ? null : "推理可调"}
        tooltip="支持 reasoning_effort 参数（low / medium / high）。OpenAI o1 / o3 / GPT-5 系列。"
        className={className}
      />
    );
  }
  return (
    <Chip
      tone="jade"
      icon={<Sparkles className="h-3 w-3" strokeWidth={2} />}
      label={compact ? null : "推理"}
      tooltip="模型支持推理 / 思考块（reasoning_content），但不强制随历史 tool_call 附带。"
      className={className}
    />
  );
}

interface ChipProps {
  tone: "rose" | "jade" | "sky";
  icon: React.ReactNode;
  label: React.ReactNode;
  tooltip: string;
  className?: string;
}

function Chip({ tone, icon, label, tooltip, className }: ChipProps) {
  const toneClass =
    tone === "rose"
      ? "bg-rose-500/[0.08] text-rose-600 dark:text-rose-400"
      : tone === "jade"
        ? "bg-jade/[0.10] text-jade"
        : "bg-sky-500/[0.08] text-sky-600 dark:text-sky-400";
  return (
    <span
      title={tooltip}
      className={cn(
        "inline-flex items-center gap-1 rounded px-1.5 py-0.5 text-[10.5px] leading-none",
        toneClass,
        className,
      )}
    >
      {icon}
      {label != null ? <span>{label}</span> : null}
    </span>
  );
}
