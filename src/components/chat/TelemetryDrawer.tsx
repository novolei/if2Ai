/**
 * TelemetryDrawer — Developer-facing observability panel.
 *
 * Provides a slide-over drawer showing live agent loop telemetry for the
 * active session, powered by the Phase 6E Harness IPC:
 *   - Token usage (input / output, total)
 *   - Tool call counts and success rates
 *   - Reflection cycles and insights
 *   - Context compaction events
 *   - Session recording status (start / stop JSONL recording)
 *
 * The drawer is intentionally developer-only — it is not shown to end
 * users by default.  Enable it by rendering `<TelemetryDrawer>` in
 * `ChatWorkspace` and connecting a "dev mode" toggle.
 *
 * Depends on Phase 6E Harness IPC (Tauri commands): `get_harness_status`,
 * `get_session_telemetry`, `start_harness_recording`, `stop_harness_recording`.
 */

import { useEffect, useRef, useState } from 'react'
import {
  Activity,
  ArrowDown,
  ArrowUp,
  CircleDot,
  Database,
  Lightbulb,
  Pin,
  PinOff,
  RefreshCw,
  Shield,
  Sparkles,
  X,
  Zap,
} from 'lucide-react'
import { cn } from '@/lib/utils'
import { useRuntimeProjectionSelector } from '@/runtime-projection'
import {
  getHarnessStatus,
  getSessionTelemetry,
  startHarnessRecording,
  stopHarnessRecording,
  type HarnessStatusResponse,
  type MemoryEventPayload,
  type SessionTelemetry,
} from '@/lib/tauri'
import { PromptDiagnosticsPanel } from '@/modules/prompt-diagnostics/components/PromptDiagnosticsPanel'
import type { PromptDiagnosticsSnapshot } from '@/modules/prompt-diagnostics/storage'

/**
 * Phase 8A.12 (T-UI-8) — bounded ring buffer of recent
 * pinned-memory / PII-redaction / rolling-summary lifecycle events.
 * Events flow in via the Tauri `memory_event` channel and are kept
 * outside React state long enough to render the timeline; older entries
 * fall off once the buffer exceeds [`MEMORY_LIFECYCLE_HISTORY_LIMIT`].
 */
const MEMORY_LIFECYCLE_HISTORY_LIMIT = 30

type MemoryLifecycleEventName =
  | 'memory_pii_redacted'
  | 'memory_pinned'
  | 'memory_unpinned'
  | 'memory_summary_rolled'

interface MemoryLifecycleLogEntry {
  uid: string
  event: MemoryLifecycleEventName
  timestamp: string
  extra?: Record<string, unknown>
  memory_category?: string
}

interface MemoryLifecycleMeta {
  Icon: typeof Pin
  tone: string
  bg: string
  label: string
  render: (entry: MemoryLifecycleLogEntry) => string
}

function asStringList(value: unknown): string[] | null {
  if (!Array.isArray(value)) return null
  const items = value.filter((v): v is string => typeof v === 'string')
  return items.length > 0 ? items : null
}

function asNumber(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) ? value : null
}

function asString(value: unknown): string | null {
  return typeof value === 'string' && value.length > 0 ? value : null
}

const MEMORY_LIFECYCLE_META: Record<MemoryLifecycleEventName, MemoryLifecycleMeta> = {
  memory_pii_redacted: {
    Icon: Shield,
    tone: 'text-red-600',
    bg: 'bg-red-50/60 border-red-200/60',
    label: '已脱敏',
    render: (entry) => {
      const detected = asStringList(entry.extra?.detected) ?? ['未知']
      return `已自动脱敏 ${detected.join(', ')}`
    },
  },
  memory_pinned: {
    Icon: Pin,
    tone: 'text-amber-600',
    bg: 'bg-amber-50/60 border-amber-200/60',
    label: '已置顶',
    render: (entry) => {
      const excerpt = asString(entry.extra?.content_excerpt) ?? ''
      return excerpt ? `已置顶："${excerpt}"` : '已置顶一条记忆'
    },
  },
  memory_unpinned: {
    Icon: PinOff,
    tone: 'text-stone-500',
    bg: 'bg-stone-50/60 border-stone-200/60',
    label: '已取消置顶',
    render: (entry) => {
      const removed = asNumber(entry.extra?.removed_count) ?? 0
      return `已取消置顶 ${removed} 条`
    },
  },
  memory_summary_rolled: {
    Icon: Sparkles,
    tone: 'text-blue-600',
    bg: 'bg-blue-50/60 border-blue-200/60',
    label: '滚动摘要',
    render: (entry) => {
      const turn = asNumber(entry.extra?.turn_count) ?? 0
      const before = asNumber(entry.extra?.chars_before) ?? 0
      const after = asNumber(entry.extra?.chars_after) ?? 0
      return `第 ${turn} 轮 · 摘要 ${before}→${after} 字`
    },
  },
}

const MEMORY_LIFECYCLE_EVENTS: ReadonlySet<MemoryLifecycleEventName> = new Set([
  'memory_pii_redacted',
  'memory_pinned',
  'memory_unpinned',
  'memory_summary_rolled',
])

function isMemoryLifecycleEvent(name: string): name is MemoryLifecycleEventName {
  return MEMORY_LIFECYCLE_EVENTS.has(name as MemoryLifecycleEventName)
}

function MemoryLifecycleRow({ entry }: { entry: MemoryLifecycleLogEntry }) {
  const meta = MEMORY_LIFECYCLE_META[entry.event]
  const time = (() => {
    try {
      return new Date(entry.timestamp).toLocaleTimeString('zh-CN', {
        hour: '2-digit',
        minute: '2-digit',
        second: '2-digit',
      })
    } catch {
      return entry.timestamp
    }
  })()
  return (
    <li className={cn('rounded-lg border px-2.5 py-1.5 text-[11px]', meta.bg)}>
      <div className="flex items-center gap-1.5">
        <meta.Icon className={cn('h-3 w-3 shrink-0', meta.tone)} aria-hidden />
        <span className={cn('text-[11px] font-semibold', meta.tone)}>{meta.label}</span>
        <span className="ml-auto font-mono text-[10px] tabular-nums text-black/40">
          {time}
        </span>
      </div>
      <div className="mt-0.5 truncate text-[11px] text-foreground/78">
        {meta.render(entry)}
      </div>
    </li>
  )
}

/**
 * Bounded ring buffer of recent memory promotion / demotion / candidate
 * events.  We keep this in component state so the drawer can show "what
 * just happened" without polling the backend; older entries are dropped
 * once we exceed [`MEMORY_EVENT_HISTORY_LIMIT`].
 */
const MEMORY_EVENT_HISTORY_LIMIT = 20

type MemoryPromoEventName =
  | 'memory_promoted'
  | 'memory_demoted'
  | 'memory_promotion_candidate'

/**
 * Snapshot of a promotion-related memory event with a client-side
 * stable id.  We intersect with `MemoryEventPayload` (rather than
 * declaring a narrower struct) so any future field added on the Rust
 * side is automatically picked up by the timeline row.
 */
type MemoryEventLogEntry = MemoryEventPayload & {
  event: MemoryPromoEventName
  uid: string
}

// ── Types ─────────────────────────────────────────────────────────────────────

export interface TelemetryDrawerProps {
  /** The session ID whose telemetry is shown. */
  sessionId: string | null
  /** Latest prompt diagnostics snapshot for the current chat session. */
  latestPromptDiagnostics: PromptDiagnosticsSnapshot | null
  /** Whether the drawer is currently open. */
  open: boolean
  /** Called when the drawer is closed. */
  onClose: () => void
  /** Additional CSS class names for the drawer panel. */
  className?: string
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/** Format a raw number with 'k' / 'M' suffixes for readability. */
function formatNumber(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`
  return String(n)
}

/** Format a millisecond duration as a human-readable string. */
function formatMs(ms: number): string {
  if (ms >= 60_000) return `${(ms / 60_000).toFixed(1)}min`
  if (ms >= 1_000) return `${(ms / 1_000).toFixed(1)}s`
  return `${ms}ms`
}

/** Compute the success rate (0–100) for a given tool from session telemetry. */
function getToolSuccessRate(telemetry: SessionTelemetry, toolName: string): number {
  const calls = telemetry.tool_calls[toolName] ?? 0
  if (calls === 0) return 0
  const successes = telemetry.tool_successes[toolName] ?? 0
  return Math.round((successes / calls) * 100)
}

// ── Sub-components ────────────────────────────────────────────────────────────

function StatRow({
  icon: Icon,
  label,
  value,
  subValue,
}: {
  icon: typeof Activity
  label: string
  value: string
  subValue?: string
}) {
  return (
    <div className="flex items-center justify-between gap-3 py-1.5">
      <div className="flex items-center gap-2 text-black/50">
        <Icon className="h-3.5 w-3.5 shrink-0" aria-hidden />
        <span className="text-[12px]">{label}</span>
      </div>
      <div className="text-right">
        <span className="text-[12px] font-semibold tabular-nums text-foreground/85">{value}</span>
        {subValue && (
          <span className="ml-1.5 text-[10.5px] text-black/40">{subValue}</span>
        )}
      </div>
    </div>
  )
}

function SectionHeader({ title }: { title: string }) {
  return (
    <div className="mb-1.5 mt-4 border-t border-black/[0.06] pt-2.5 text-[10px] font-semibold uppercase tracking-widest text-black/35">
      {title}
    </div>
  )
}

/**
 * Single row of the memory promotion / demotion / candidate timeline.
 *
 * Visual language:
 *   - promoted   → emerald arrow-up   (write happened)
 *   - demoted    → amber  arrow-down  (write happened)
 *   - candidate  → violet sparkle     (advisory only, no write)
 *
 * Tier transitions are pulled from the audit payload's
 * `from_category` / `to_category` slots — the backend reuses those fields
 * to carry tier names so we don't have to widen `MemoryEventPayload`.
 */
type MemoryTimelineMeta = {
  Icon: typeof ArrowUp
  tone: string
  bg: string
  label: string
}

const MEMORY_TIMELINE_META: Record<MemoryPromoEventName, MemoryTimelineMeta> = {
  memory_promoted: {
    Icon: ArrowUp,
    tone: 'text-emerald-600',
    bg: 'bg-emerald-50/60 border-emerald-200/60',
    label: '已晋升',
  },
  memory_demoted: {
    Icon: ArrowDown,
    tone: 'text-amber-600',
    bg: 'bg-amber-50/60 border-amber-200/60',
    label: '已降级',
  },
  memory_promotion_candidate: {
    Icon: Sparkles,
    tone: 'text-violet-600',
    bg: 'bg-violet-50/60 border-violet-200/60',
    label: '候选晋升',
  },
}

function MemoryTimelineItem({ event }: { event: MemoryEventLogEntry }) {
  const meta = MEMORY_TIMELINE_META[event.event]

  const fromTier = event.from_category ?? '?'
  const toTier = event.to_category ?? '?'
  const time = (() => {
    try {
      return new Date(event.timestamp).toLocaleTimeString('zh-CN', {
        hour: '2-digit',
        minute: '2-digit',
        second: '2-digit',
      })
    } catch {
      return event.timestamp
    }
  })()

  return (
    <li className={cn('rounded-lg border px-2.5 py-1.5 text-[11px]', meta.bg)}>
      <div className="flex items-center gap-1.5">
        <meta.Icon className={cn('h-3 w-3 shrink-0', meta.tone)} aria-hidden />
        <span className={cn('text-[11px] font-semibold', meta.tone)}>{meta.label}</span>
        <span className="ml-auto font-mono text-[10px] tabular-nums text-black/40">
          {time}
        </span>
      </div>
      <div className="mt-0.5 truncate text-[11px] text-foreground/78">
        <span className="font-mono">{event.memory_key ?? '(no key)'}</span>
      </div>
      <div className="mt-0.5 flex items-center gap-1.5 text-[10.5px] text-black/50">
        <span className="font-mono">{fromTier}</span>
        <span aria-hidden>→</span>
        <span className="font-mono">{toTier}</span>
        {event.reason_message && (
          <span className="ml-auto truncate" title={event.reason_message}>
            {event.reason_message}
          </span>
        )}
      </div>
    </li>
  )
}

// ── TelemetryDrawer ───────────────────────────────────────────────────────────

/**
 * Slide-over developer drawer showing live session telemetry and recording controls.
 */
export function TelemetryDrawer({
  sessionId,
  latestPromptDiagnostics,
  open,
  onClose,
  className,
}: TelemetryDrawerProps) {
  const [harness, setHarness] = useState<HarnessStatusResponse | null>(null)
  const [telemetry, setTelemetry] = useState<SessionTelemetry | null>(null)
  const [loading, setLoading] = useState(false)
  const [recordingBusy, setRecordingBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [memoryLifecycleLog, setMemoryLifecycleLog] = useState<MemoryLifecycleLogEntry[]>([])
  const memoryLifecycleUidRef = useRef(0)
  const [memoryLog, setMemoryLog] = useState<MemoryEventLogEntry[]>([])
  const memoryUidCounter = useRef(0)

  // Phase M2.8 — consume canonical projection store instead of
  // a direct `listenMemoryEvent` subscription.  The projection
  // store buffers up to 64 most-recent memory events (see
  // `MemoryRollingProjection.recentEvents` in the runtime-projection
  // types); we replay the slice that arrived since the last
  // dispatch into the existing categorised local logs so the
  // drawer keeps its richer per-category history (lifecycle 100 /
  // promotion 200) and existing render logic untouched.
  const recentMemoryEvents = useRuntimeProjectionSelector(
    (s) => s.memory.recentEvents,
  )
  const lastSeenMemoryEventCountRef = useRef(0)
  useEffect(() => {
    const total = recentMemoryEvents.length
    const seen = lastSeenMemoryEventCountRef.current
    // Detect a ring rotation (older events evicted): if the
    // counter exceeds the current array length, snap back to 0
    // and treat the whole snapshot as fresh.  Otherwise process
    // only the suffix that appeared since last render.
    const newSliceStart = seen > total ? 0 : seen
    const fresh = recentMemoryEvents.slice(newSliceStart)
    lastSeenMemoryEventCountRef.current = total
    if (fresh.length === 0) return
    for (const payload of fresh) {
      if (isMemoryLifecycleEvent(payload.event)) {
        memoryLifecycleUidRef.current += 1
        const entry: MemoryLifecycleLogEntry = {
          uid: `${payload.timestamp}-${memoryLifecycleUidRef.current}`,
          event: payload.event,
          timestamp: payload.timestamp,
          extra: payload.extra,
          memory_category: payload.memory_category,
        }
        setMemoryLifecycleLog((prev) => {
          const next = [entry, ...prev]
          return next.length > MEMORY_LIFECYCLE_HISTORY_LIMIT
            ? next.slice(0, MEMORY_LIFECYCLE_HISTORY_LIMIT)
            : next
        })
        continue
      }
      if (
        payload.event !== 'memory_promoted' &&
        payload.event !== 'memory_demoted' &&
        payload.event !== 'memory_promotion_candidate'
      ) {
        continue
      }
      memoryUidCounter.current += 1
      const promoEntry: MemoryEventLogEntry = {
        ...payload,
        event: payload.event,
        uid: `${payload.timestamp}-${memoryUidCounter.current}`,
      }
      setMemoryLog((prev) => {
        const next = [promoEntry, ...prev]
        return next.length > MEMORY_EVENT_HISTORY_LIMIT
          ? next.slice(0, MEMORY_EVENT_HISTORY_LIMIT)
          : next
      })
    }
  }, [recentMemoryEvents])

  const isRecording = harness !== null &&
    sessionId !== null &&
    harness.active_recordings.includes(sessionId)

  // Refresh telemetry when the drawer opens or sessionId changes.
  useEffect(() => {
    if (!open || !sessionId) return

    let cancelled = false

    const refresh = async () => {
      setLoading(true)
      setError(null)
      try {
        const [statusRes, telemetryRes] = await Promise.all([
          getHarnessStatus(),
          getSessionTelemetry(sessionId),
        ])
        if (!cancelled) {
          setHarness(statusRes)
          setTelemetry(telemetryRes.found ? telemetryRes.telemetry : null)
        }
      } catch (err) {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : String(err))
        }
      } finally {
        if (!cancelled) setLoading(false)
      }
    }

    void refresh()

    // Poll every 5 seconds while the drawer is open.
    const interval = window.setInterval(() => void refresh(), 5_000)
    return () => {
      cancelled = true
      clearInterval(interval)
    }
  }, [open, sessionId])

  const handleToggleRecording = async () => {
    if (!sessionId) return
    setRecordingBusy(true)
    try {
      if (isRecording) {
        await stopHarnessRecording(sessionId)
      } else {
        await startHarnessRecording(sessionId)
      }
      // Refresh status after toggling.
      const statusRes = await getHarnessStatus()
      setHarness(statusRes)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setRecordingBusy(false)
    }
  }

  if (!open) return null

  // Sorted tool list by call count, descending.
  const toolEntries = telemetry
    ? Object.entries(telemetry.tool_calls).sort(([, a], [, b]) => b - a)
    : []

  const avgTurnMs = telemetry && telemetry.turns_completed > 0
    ? Math.round(telemetry.total_turn_duration_ms / telemetry.turns_completed)
    : 0

  return (
    <>
      {/* Backdrop */}
      <div
        className="fixed inset-0 z-40 bg-black/10"
        aria-hidden
        onClick={onClose}
      />

      {/* Drawer panel */}
      <aside
        role="complementary"
        aria-label="开发者遥测面板"
        className={cn(
          'fixed right-0 top-0 z-50 flex h-full w-[26rem] flex-col',
          'border-l border-black/[0.07] bg-white shadow-[0_0_48px_rgba(15,23,42,0.10)]',
          className,
        )}
      >
        {/* Header */}
        <div className="flex items-center justify-between border-b border-black/[0.06] bg-black/[0.012] px-4 py-3">
          <div className="flex items-center gap-2">
            <span className="inline-flex h-7 w-7 items-center justify-center rounded-lg bg-jade/10 text-jade">
              <Activity className="h-3.5 w-3.5" aria-hidden />
            </span>
            <div className="leading-tight">
              <div className="text-[12.5px] font-semibold tracking-tight text-foreground/85">
                遥测面板
              </div>
              <div className="text-[10px] text-black/40">Developer telemetry</div>
            </div>
            {loading && (
              <RefreshCw className="ml-1 h-3 w-3 animate-spin text-black/40" aria-label="加载中" />
            )}
          </div>
          <button
            type="button"
            onClick={onClose}
            className="rounded-md p-1.5 text-black/45 transition-colors hover:bg-black/[0.04] hover:text-foreground"
            aria-label="关闭遥测面板"
          >
            <X className="h-3.5 w-3.5" />
          </button>
        </div>

        {/* Scrollable content */}
        <div className="flex-1 overflow-y-auto px-4 pb-5">
          {/* Error state */}
          {error && (
            <div className="mt-3 rounded-lg border border-destructive/20 bg-destructive/8 px-3 py-2 text-[11px] leading-4 text-destructive">
              {error}
            </div>
          )}

          {/* Harness status */}
          {harness && (
            <div className="mt-3 flex items-center justify-between rounded-lg border border-black/[0.06] bg-black/[0.015] px-3 py-2">
              <div className="flex items-center gap-1.5 text-[11px] text-black/55">
                <CircleDot
                  className={cn(
                    'h-3 w-3',
                    harness.harness_enabled ? 'text-status-success' : 'text-black/30',
                  )}
                  aria-hidden
                />
                <span className="font-medium">
                  {harness.harness_enabled ? 'Harness 已启用' : 'Harness 未启用'}
                </span>
              </div>

              {harness.harness_enabled && sessionId && (
                <button
                  type="button"
                  onClick={() => void handleToggleRecording()}
                  disabled={recordingBusy || !sessionId}
                  className={cn(
                    'rounded-md border px-2 py-0.5 text-[10px] font-medium transition-colors',
                    isRecording
                      ? 'border-status-error/30 bg-status-error-bg text-status-error hover:bg-status-error/15'
                      : 'border-status-success/30 bg-status-success-bg text-status-success hover:bg-status-success/15',
                    (recordingBusy || !sessionId) && 'cursor-default opacity-50',
                  )}
                  aria-label={isRecording ? '停止录制' : '开始录制'}
                >
                  {recordingBusy ? '…' : isRecording ? '停止录制' : '开始录制'}
                </button>
              )}
            </div>
          )}

          {/* No data */}
          {!loading && !telemetry && !error && (
            <div className="mt-12 text-center text-[12px] text-black/45">
              {sessionId ? '当前会话暂无遥测数据' : '请选择一个会话'}
            </div>
          )}

          {/* Telemetry data */}
          {telemetry && (
            <>
              {/* Session overview */}
              <SectionHeader title="会话概览" />
              <StatRow
                icon={Activity}
                label="完成轮次"
                value={String(telemetry.turns_completed)}
                subValue={telemetry.turns_succeeded < telemetry.turns_completed
                  ? `${telemetry.turns_succeeded} 成功`
                  : undefined}
              />
              <StatRow
                icon={Zap}
                label="LLM 调用"
                value={String(telemetry.llm_calls)}
                subValue={avgTurnMs > 0 ? `均 ${formatMs(avgTurnMs)}/轮` : undefined}
              />

              {/* Token usage */}
              <SectionHeader title="Token 用量" />
              <StatRow
                icon={Database}
                label="输入"
                value={formatNumber(telemetry.input_tokens_total)}
              />
              <StatRow
                icon={Database}
                label="输出"
                value={formatNumber(telemetry.output_tokens_total)}
              />
              <StatRow
                icon={Database}
                label="合计"
                value={formatNumber(telemetry.input_tokens_total + telemetry.output_tokens_total)}
              />

              {/* Tool calls */}
              {toolEntries.length > 0 && (
                <>
                  <SectionHeader title="工具调用" />
                  {toolEntries.map(([toolName, count]) => (
                    <StatRow
                      key={toolName}
                      icon={Activity}
                      label={toolName}
                      value={String(count)}
                      subValue={`${getToolSuccessRate(telemetry, toolName)}% 成功`}
                    />
                  ))}
                </>
              )}

              {/* Learning + memory */}
              <SectionHeader title="学习 / 记忆" />
              <StatRow
                icon={Lightbulb}
                label="反思周期"
                value={String(telemetry.reflection_cycles)}
                subValue={`${telemetry.reflection_insights_total} 条洞见`}
              />
              <StatRow
                icon={Database}
                label="上下文压缩"
                value={String(telemetry.compaction_events)}
              />
              <StatRow
                icon={Activity}
                label="权限请求"
                value={String(telemetry.permission_prompts)}
              />

              {/* Last event */}
              {telemetry.last_event_at && (
                <div className="mt-3 rounded-md border border-black/[0.06] bg-black/[0.015] px-2.5 py-1.5 text-[10px] text-black/45">
                  最后事件：{new Date(telemetry.last_event_at).toLocaleTimeString('zh-CN')}
                </div>
              )}
            </>
          )}

          <SectionHeader title="Prompt Diagnostics" />
          <div className="mt-1.5">
            <PromptDiagnosticsPanel
              snapshot={latestPromptDiagnostics}
              title="Prompt Diagnostics"
              description="当前会话最近一次 assistant 完成回合的 prompt control plane 摘要。"
              collapsible
              defaultExpanded={false}
              compact
            />
          </div>

          {/* Memory lifecycle timeline (Phase 8A.12 / T-UI-8) */}
          {memoryLifecycleLog.length > 0 && (
            <>
              <SectionHeader title="记忆生命周期" />
              <ul className="mt-1.5 space-y-1.5">
                {memoryLifecycleLog.map((entry) => (
                  <MemoryLifecycleRow key={entry.uid} entry={entry} />
                ))}
              </ul>
            </>
          )}

          {/* Memory promotion / demotion timeline.  Always rendered (even
              without telemetry) because the listener captures events
              process-wide. */}
          <SectionHeader title="记忆晋升 / 降级" />
          {memoryLog.length === 0 ? (
            <div className="px-1 py-1.5 text-[11px] text-black/40">
              暂无晋升或降级事件
            </div>
          ) : (
            <ul className="flex flex-col gap-1.5">
              {memoryLog.map((evt) => (
                <MemoryTimelineItem key={evt.uid} event={evt} />
              ))}
            </ul>
          )}
        </div>
      </aside>
    </>
  )
}
