/**
 * FinalRunReportCard — terminal status banner for an assistant run.
 *
 * Extracted from `src/components/ui/chat-ui.tsx` (GF-01 PR-01) without
 * behaviour change. Outcome → palette mapping, copy, and icon choices
 * match the legacy declaration verbatim.
 */

import * as React from 'react'
import { AlertTriangle, Check, Clock3, Lock } from 'lucide-react'
import { cn } from '@/lib/utils'
import type { FinalRunReport } from '@/transport/contracts'

/**
 * Render the canonical AWL-004 final run report card. The optional
 * `onResume` prop is forwarded by callers but currently unused — the
 * legacy implementation reserved it for a future resume button.
 */
export function FinalRunReportCard({
  report,
}: {
  report: FinalRunReport
  onResume?: (resumeCursor: string) => void
}): React.JSX.Element {
  const meta = finalReportMeta(report)

  return (
    <div className={cn('w-full overflow-hidden rounded-[6px] border px-3 py-2', meta.containerClass)}>
      <div className="flex items-center gap-2">
        <span className={cn('inline-flex h-6 w-6 shrink-0 items-center justify-center rounded-full', meta.iconClass)}>
          {meta.kind === 'done' ? (
            <Check className="h-3.5 w-3.5" />
          ) : meta.kind === 'approval' ? (
            <Lock className="h-3.5 w-3.5" />
          ) : meta.kind === 'exhausted' ? (
            <Clock3 className="h-3.5 w-3.5" />
          ) : (
            <AlertTriangle className="h-3.5 w-3.5" />
          )}
        </span>
        <div className="min-w-0 flex-1 truncate">
          <span className={cn('text-[12.5px] font-semibold leading-4.5', meta.titleClass)}>
            {meta.title}
          </span>
          <span className="ml-2 text-[11.5px] leading-4.5 text-muted-foreground">
            Details are available in Developer telemetry.
          </span>
        </div>
        <span className="shrink-0 rounded-md border border-border/55 bg-background/45 px-1.5 py-0.5 text-[10.5px] leading-4 text-muted-foreground">
          {humanizeRuntimeValue(report.loopKind)}
        </span>
      </div>
    </div>
  )
}

/**
 * Resolve the visual palette + title for a FinalRunReport's outcome.
 * Returns the legacy `done | approval | exhausted | failed` mapping.
 */
function finalReportMeta(report: FinalRunReport): {
  kind: 'done' | 'approval' | 'exhausted' | 'failed'
  title: string
  containerClass: string
  iconClass: string
  titleClass: string
} {
  if (report.outcome === 'completed') {
    return {
      kind: 'done',
      title: 'Run completed',
      containerClass: 'border-emerald-500/35 bg-emerald-500/8',
      iconClass: 'bg-emerald-500/14 text-emerald-500',
      titleClass: 'text-emerald-500',
    }
  }
  if (report.outcome === 'needs_approval') {
    return {
      kind: 'approval',
      title: 'Waiting for approval',
      containerClass: 'border-amber-500/35 bg-amber-500/10',
      iconClass: 'bg-amber-500/14 text-amber-500',
      titleClass: 'text-amber-500',
    }
  }
  if (report.outcome === 'exhausted_with_summary') {
    return {
      kind: 'exhausted',
      title: 'Stopped after limits',
      containerClass: 'border-sky-500/35 bg-sky-500/10',
      iconClass: 'bg-sky-500/14 text-sky-500',
      titleClass: 'text-sky-500',
    }
  }
  return {
    kind: 'failed',
    title: 'Could not finish',
    containerClass: 'border-rose-500/35 bg-rose-500/10',
    iconClass: 'bg-rose-500/14 text-rose-500',
    titleClass: 'text-rose-500',
  }
}

/**
 * Convert a snake_case runtime value to a space-separated label, or
 * the literal "pending" when undefined.
 */
function humanizeRuntimeValue(value: string | undefined): string {
  if (!value) return 'pending'
  return value.replace(/_/g, ' ')
}
