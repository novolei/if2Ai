import { Route, Sparkles } from "lucide-react";

import type { RoutingInfo } from "@/runtime-projection/types";
import { cn } from "@/lib/utils";

interface RoutingChipProps {
  routing: RoutingInfo;
  className?: string;
}

function complexityToneClass(level: string): string {
  switch (level.toLowerCase()) {
    case "low":
    case "trivial":
      return "text-muted-foreground";
    case "moderate":
    case "medium":
      return "text-amber-600 dark:text-amber-400";
    case "complex":
    case "high":
      return "text-rose-600 dark:text-rose-400";
    default:
      return "text-muted-foreground";
  }
}

function complexityLabel(level: string): string {
  switch (level.toLowerCase()) {
    case "low":
    case "trivial":
      return "低复杂度";
    case "moderate":
    case "medium":
      return "中复杂度";
    case "complex":
    case "high":
      return "高复杂度";
    default:
      return level;
  }
}

/**
 * P1-8 — per-turn smart-routing chip rendered below an assistant message.
 * Shows the complexity bucket (color-coded) + a sparkle when a cheap model
 * was selected.  Tooltip surfaces the effective model id + raw score.
 */
export function RoutingChip({ routing, className }: RoutingChipProps) {
  const score = Number.isFinite(routing.complexityScore)
    ? routing.complexityScore.toFixed(2)
    : "?";
  const tooltip = [
    `effective model: ${routing.effectiveModel || "unknown"}`,
    `complexity: ${routing.complexityLevel} (${score})`,
    `execution mode: ${routing.executionMode}`,
    routing.usedCheapModel ? "smart routing → cheap model" : null,
  ]
    .filter(Boolean)
    .join("\n");
  return (
    <div
      title={tooltip}
      className={cn(
        "inline-flex items-center gap-1 rounded px-1.5 py-0.5 text-[11px] leading-none",
        complexityToneClass(routing.complexityLevel),
        className,
      )}
    >
      {routing.usedCheapModel ? (
        <Sparkles className="h-3 w-3" strokeWidth={2} />
      ) : (
        <Route className="h-3 w-3" strokeWidth={2} />
      )}
      <span>{complexityLabel(routing.complexityLevel)}</span>
    </div>
  );
}
