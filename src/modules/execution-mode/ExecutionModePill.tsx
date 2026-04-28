// Phase M2.6 — minimal, honest visible surface for the runtime
// execution-mode projection.
//
// Renders a small status pill summarising the **classifier's
// current judgment** of the latest classified draft. Honest scope:
//
//   - Pill only shows when `snapshot.executionMode != null`.
//     Today that requires someone to call
//     `refreshExecutionModeDecision(...)` (e.g. via
//     `useExecutionModePreview`).
//   - Copy is explicitly "判定" (judgment), never "已切换"
//     (auto-routed). The deterministic classifier in
//     `request_intelligence_service` is **advisory** — the agent
//     loop continues to run its single existing execution path
//     regardless.
//   - Only fields the canonical `ExecutionModeProjection` actually
//     carries are rendered. No invented metadata.
//   - `risk_level === 'high'` raises a contrast accent so users get
//     a quick visual cue, but the pill never blocks input or
//     rewrites the conversation flow.
//
// Out of scope (stays for m2.7+):
//   - Manual override controls (see runbook §5.4 — `manualOverride`
//     belongs to the future execution-mode store, not the pill).
//   - Reason-code / rule-id tooltips. Today the projection carries
//     `reasonCodes`; a richer explainability popover lives in a
//     future slice once the rule-id catalogue stabilises.
//   - Specialised surface routing. The pill is a status indicator,
//     not a router.

import {
  useRuntimeProjectionSelector,
  type ExecutionModeProjection,
} from '@/runtime-projection'

const MODE_LABEL: Record<ExecutionModeProjection['executionMode'], string> = {
  direct_execute: '直接执行',
  auto_plan_execute: '自动多步',
  plan_then_confirm: '需确认计划',
  specialized_surface: '专用面板',
}

const RISK_LABEL: Record<ExecutionModeProjection['riskLevel'], string> = {
  low: '低风险',
  medium: '中风险',
  high: '高风险',
}

const COMPLEXITY_LABEL: Record<
  ExecutionModeProjection['complexityLevel'],
  string
> = {
  trivial: '极简',
  simple: '简单',
  moderate: '一般',
  complex: '复杂',
}

export interface ExecutionModePillProps {
  /** Optional className override for layout integration. */
  className?: string
}

export function ExecutionModePill({ className }: ExecutionModePillProps) {
  const projection = useRuntimeProjectionSelector((s) => s.executionMode)
  if (!projection) {
    return null
  }

  const modeLabel = MODE_LABEL[projection.executionMode] ?? projection.executionMode
  const riskLabel = RISK_LABEL[projection.riskLevel] ?? projection.riskLevel
  const complexityLabel = COMPLEXITY_LABEL[projection.complexityLevel] ?? projection.complexityLevel
  const reasonCodes = Array.isArray(projection.reasonCodes)
    ? projection.reasonCodes.filter((reason): reason is string => typeof reason === 'string')
    : []
  const high = projection.riskLevel === 'high'
  const baseClass =
    'inline-flex max-w-full flex-wrap items-center gap-2 rounded-full border px-2.5 py-0.5 text-[11px] font-medium leading-none'
  const accentClass = high
    ? 'border-amber-300/80 bg-amber-50 text-amber-900 dark:border-amber-500/40 dark:bg-amber-950/40 dark:text-amber-200'
    : 'border-zinc-300/80 bg-zinc-100 text-zinc-700 dark:border-zinc-700 dark:bg-zinc-800/60 dark:text-zinc-300'

  const tooltip = [
    '当前判定：',
    `模式 ${modeLabel}`,
    `风险 ${riskLabel}`,
    `复杂度 ${complexityLabel}`,
    `分类策略 ${projection.policyVersion ?? 'unknown'}`,
    reasonCodes.length > 0
      ? `原因 ${reasonCodes.join(' / ')}`
      : null,
    '注：判定仅为建议，未自动切换执行通道。',
  ]
    .filter(Boolean)
    .join('\n')

  return (
    <span
      className={`${baseClass} ${accentClass} ${className ?? ''}`.trim()}
      title={tooltip}
      aria-label={tooltip}
    >
      <span className="opacity-70">判定</span>
      <span>{modeLabel}</span>
      <span className="opacity-60">·</span>
      <span>{riskLabel}</span>
      <span className="opacity-60">·</span>
      <span>{complexityLabel}</span>
    </span>
  )
}
