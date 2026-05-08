/**
 * GF-01 PR-07 — extracted from `src/components/ui/chat-ui.tsx`.
 *
 * Thin presentational mount around `@/components/chat/ContextBar`.
 * Owns the responsive max-width + right-inset chrome only; the budget
 * snapshot / session totals derivation stays in `ChatUI`.
 *
 * Render-equivalent move from the original inline JSX.
 */

import * as React from "react"
import { ContextBar } from "@/components/chat/ContextBar"
import type { ContextBudgetUsage, SessionTotals } from "@/lib/tauri"

/**
 * Mount the context-bar (manual /compact + per-session totals) inside
 * the chat surface. Returns `null` when no `usage` snapshot exists yet
 * so callers can unconditionally render `<ContextBarMount usage={…} />`.
 */
export function ContextBarMount({
  usage,
  windowSize,
  sessionTotals,
  sessionId,
  composerMaxWidth,
  chatVisibleRightInset,
}: {
  usage: ContextBudgetUsage | undefined
  windowSize: number
  sessionTotals: SessionTotals | undefined
  sessionId: string | undefined
  composerMaxWidth: number
  chatVisibleRightInset: number
}) {
  if (!usage) return null
  return (
    <div
      className="relative z-0 shrink-0 px-10 transition-[padding-right] duration-300 ease-[cubic-bezier(0.22,1,0.36,1)]"
      style={{ paddingRight: `${40 + chatVisibleRightInset}px` }}
    >
      <div
        className="mx-auto w-full transition-[max-width] duration-300 ease-[cubic-bezier(0.22,1,0.36,1)]"
        style={{ maxWidth: `${composerMaxWidth}px` }}
      >
        <ContextBar
          usage={usage}
          windowSize={windowSize}
          sessionTotals={sessionTotals}
          sessionId={sessionId}
        />
      </div>
    </div>
  )
}
