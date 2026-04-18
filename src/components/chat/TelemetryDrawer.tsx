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

import { useEffect, useState } from 'react'
import { Activity, CircleDot, Database, Lightbulb, RefreshCw, X, Zap } from 'lucide-react'
import { cn } from '@/lib/utils'
import {
  getHarnessStatus,
  getSessionTelemetry,
  startHarnessRecording,
  stopHarnessRecording,
  type HarnessStatusResponse,
  type SessionTelemetry,
} from '@/lib/tauri'

// ── Types ─────────────────────────────────────────────────────────────────────

export interface TelemetryDrawerProps {
  /** The session ID whose telemetry is shown. */
  sessionId: string | null
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
      <div className="flex items-center gap-2 text-muted-foreground">
        <Icon className="h-3.5 w-3.5 shrink-0" aria-hidden />
        <span className="text-[12px]">{label}</span>
      </div>
      <div className="text-right">
        <span className="text-[12px] font-medium tabular-nums text-foreground/80">{value}</span>
        {subValue && (
          <span className="ml-1.5 text-[11px] text-muted-foreground">{subValue}</span>
        )}
      </div>
    </div>
  )
}

function SectionHeader({ title }: { title: string }) {
  return (
    <div className="mb-1 mt-3 border-t border-border/50 pt-2 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground/60">
      {title}
    </div>
  )
}

// ── TelemetryDrawer ───────────────────────────────────────────────────────────

/**
 * Slide-over developer drawer showing live session telemetry and recording controls.
 */
export function TelemetryDrawer({ sessionId, open, onClose, className }: TelemetryDrawerProps) {
  const [harness, setHarness] = useState<HarnessStatusResponse | null>(null)
  const [telemetry, setTelemetry] = useState<SessionTelemetry | null>(null)
  const [loading, setLoading] = useState(false)
  const [recordingBusy, setRecordingBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)

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
          'fixed right-0 top-0 z-50 flex h-full w-72 flex-col',
          'border-l border-border bg-popover shadow-token-lg',
          className,
        )}
      >
        {/* Header */}
        <div className="flex items-center justify-between border-b border-border px-4 py-3">
          <div className="flex items-center gap-2">
            <Activity className="h-4 w-4 text-primary" aria-hidden />
            <span className="text-[13px] font-semibold text-foreground/80">遥测面板</span>
            {loading && (
              <RefreshCw className="h-3 w-3 animate-spin text-muted-foreground" aria-label="加载中" />
            )}
          </div>
          <button
            type="button"
            onClick={onClose}
            className="rounded p-1 text-muted-foreground hover:bg-accent hover:text-foreground"
            aria-label="关闭遥测面板"
          >
            <X className="h-3.5 w-3.5" />
          </button>
        </div>

        {/* Scrollable content */}
        <div className="flex-1 overflow-y-auto px-4 pb-4">
          {/* Error state */}
          {error && (
            <div className="mt-3 rounded-md bg-destructive/10 px-3 py-2 text-[11px] text-destructive">
              {error}
            </div>
          )}

          {/* Harness status */}
          {harness && (
            <div className="mt-3 flex items-center justify-between">
              <div className="flex items-center gap-1.5 text-[11px] text-muted-foreground">
                <CircleDot
                  className={cn('h-3 w-3', harness.harness_enabled ? 'text-status-success' : 'text-muted-foreground/40')}
                  aria-hidden
                />
                <span>{harness.harness_enabled ? 'Harness 已启用' : 'Harness 未启用'}</span>
              </div>

              {harness.harness_enabled && sessionId && (
                <button
                  type="button"
                  onClick={() => void handleToggleRecording()}
                  disabled={recordingBusy || !sessionId}
                  className={cn(
                    'rounded-full px-2.5 py-0.5 text-[10px] font-medium transition-colors',
                    isRecording
                      ? 'bg-status-error-bg text-status-error hover:bg-status-error/20'
                      : 'bg-status-success-bg text-status-success hover:bg-status-success/20',
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
            <div className="mt-8 text-center text-[12px] text-muted-foreground">
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
                <div className="mt-3 text-[10px] text-muted-foreground/60">
                  最后事件：{new Date(telemetry.last_event_at).toLocaleTimeString('zh-CN')}
                </div>
              )}
            </>
          )}
        </div>
      </aside>
    </>
  )
}
