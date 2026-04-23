import { Zap } from "lucide-react";

import type { TurnCost } from "@/runtime-projection/types";
import { cn } from "@/lib/utils";

interface TurnCostChipProps {
  turnCost: TurnCost;
  className?: string;
}

function formatCost(cost: number): string {
  if (cost <= 0) return "$0.0000";
  if (cost < 0.0001) return "<$0.0001";
  return `$${cost.toFixed(4)}`;
}

/**
 * Per-turn token + USD chip rendered below an assistant message.
 * Mirrors Steward's `turn-cost-bar` (see ChatArea.svelte): icon + input
 * tokens + output tokens + cost (cost segment hidden when 0).
 */
export function TurnCostChip({ turnCost, className }: TurnCostChipProps) {
  const cost = formatCost(turnCost.costUsd);
  const showCost = cost !== "$0.0000";
  const tooltip = [
    `model: ${turnCost.model || "unknown"}`,
    `input: ${turnCost.inputTokens.toLocaleString()}`,
    `output: ${turnCost.outputTokens.toLocaleString()}`,
    turnCost.cacheReadInputTokens > 0
      ? `cache read: ${turnCost.cacheReadInputTokens.toLocaleString()}`
      : null,
    turnCost.cacheCreationInputTokens > 0
      ? `cache write: ${turnCost.cacheCreationInputTokens.toLocaleString()}`
      : null,
    `cost: ${cost}`,
  ]
    .filter(Boolean)
    .join("\n");
  return (
    <div
      title={tooltip}
      className={cn(
        "inline-flex items-center gap-1.5 rounded px-1.5 py-0.5 text-[11px] leading-none text-muted-foreground",
        className,
      )}
    >
      <Zap className="h-3 w-3" strokeWidth={2} />
      <span className="tabular-nums">
        {turnCost.inputTokens.toLocaleString()} 输入
      </span>
      <span className="text-muted-foreground/60">·</span>
      <span className="tabular-nums">
        {turnCost.outputTokens.toLocaleString()} 输出
      </span>
      {showCost ? (
        <>
          <span className="text-muted-foreground/60">·</span>
          <span className="tabular-nums">{cost}</span>
        </>
      ) : null}
    </div>
  );
}
