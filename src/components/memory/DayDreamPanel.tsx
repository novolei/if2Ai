/**
 * DayDreamPanel — 记忆巩固可视化面板
 *
 * 显示 DayDream 引擎状态、快速操作、配置和历史报告。
 * 用于 MemorySettingsPage 的 "巩固" tab。
 */

import { useState } from 'react'
import { toast } from 'sonner'
import {
  ChevronDown,
  ChevronRight,
  Moon,
  Play,
  RefreshCw,
  AlertTriangle,
  Settings2,
  History,
  Scissors,
  GitMerge,
  Sparkles,
} from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Switch } from '@/components/ui/switch'
import {
  useDayDreamStatus,
  useDayDreamConfig,
  useDayDreamHistory,
  daydreamTrigger,
  type DayDreamReport,
  type ConsolidationStrategy,
} from '@/api/memory'

// ── Status indicator helpers ──────────────────────────────────────────

function StatusDot({ kind }: { kind: string }) {
  switch (kind) {
    case 'Running':
      return (
        <span className="relative flex h-2.5 w-2.5">
          <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-blue-400 opacity-75" />
          <span className="relative inline-flex h-2.5 w-2.5 rounded-full bg-blue-500" />
        </span>
      )
    case 'Completed':
      return <span className="inline-block h-2.5 w-2.5 rounded-full bg-emerald-500" />
    case 'Disabled':
      return <span className="inline-block h-2.5 w-2.5 rounded-full bg-red-400" />
    default: // Idle
      return <span className="inline-block h-2.5 w-2.5 rounded-full bg-gray-400" />
  }
}

function statusLabel(kind: string, finishedAt?: string): string {
  switch (kind) {
    case 'Running':
      return '巩固中...'
    case 'Completed': {
      if (finishedAt) {
        try {
          const d = new Date(finishedAt)
          return `最近完成于 ${d.toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' })}`
        } catch {
          return '已完成'
        }
      }
      return '已完成'
    }
    case 'Disabled':
      return '已禁用'
    default:
      return '空闲中'
  }
}

const STRATEGY_OPTIONS: ReadonlyArray<{ value: ConsolidationStrategy; label: string; desc: string }> = [
  { value: 'Conservative', label: '保守', desc: '仅修剪明显过时项' },
  { value: 'Balanced', label: '均衡', desc: '修剪 + 合并相似项' },
  { value: 'Aggressive', label: '积极', desc: '修剪 + 合并 + 主动刷新' },
]

// ── Report card ───────────────────────────────────────────────────────

function ReportCard({ report }: { report: DayDreamReport }) {
  const hasErrors = report.errors.length > 0
  const startTime = (() => {
    try {
      return new Date(report.started_at).toLocaleString('zh-CN', {
        month: '2-digit',
        day: '2-digit',
        hour: '2-digit',
        minute: '2-digit',
      })
    } catch {
      return report.started_at
    }
  })()

  return (
    <div
      className={`rounded-lg border px-3 py-2.5 text-[12px] ${
        hasErrors
          ? 'border-red-200/70 bg-red-50/50'
          : 'border-border/60 bg-card/60'
      }`}
    >
      <div className="flex items-center justify-between gap-2">
        <span className="font-medium text-foreground/90">{startTime}</span>
        <div className="flex items-center gap-2 text-[11px] text-muted-foreground">
          <span className="rounded bg-muted/50 px-1.5 py-0.5">
            {report.strategy === 'Conservative' ? '保守' : report.strategy === 'Balanced' ? '均衡' : '积极'}
          </span>
          <span>{(report.duration_ms / 1000).toFixed(1)}s</span>
        </div>
      </div>

      <div className="mt-1.5 flex items-center gap-3 text-[11px] text-muted-foreground">
        <span className="flex items-center gap-1">
          <Scissors className="h-3 w-3" />
          修剪 {report.prune.pruned}/{report.prune.scanned}
        </span>
        <span className="flex items-center gap-1">
          <GitMerge className="h-3 w-3" />
          合并 {report.merge.merged} 组（消耗 {report.merge.entries_consumed}）
        </span>
        <span className="flex items-center gap-1">
          <Sparkles className="h-3 w-3" />
          刷新 {report.refresh.refreshed}/{report.refresh.candidates}
        </span>
      </div>

      {hasErrors && (
        <div className="mt-1.5 flex items-start gap-1.5 text-[11px] text-red-600">
          <AlertTriangle className="mt-0.5 h-3 w-3 shrink-0" />
          <span>{report.errors.join('; ')}</span>
        </div>
      )}
    </div>
  )
}

// ── Main panel ────────────────────────────────────────────────────────

export function DayDreamPanel() {
  const { parsed, loading: statusLoading, error: statusError, refetch: refetchStatus } = useDayDreamStatus()
  const { config, loading: configLoading, error: configError, update: updateConfig } = useDayDreamConfig()
  const { reports, loading: historyLoading, error: historyError, refetch: refetchHistory } = useDayDreamHistory()

  const [triggering, setTriggering] = useState(false)
  const [configOpen, setConfigOpen] = useState(false)
  const [historyOpen, setHistoryOpen] = useState(false)

  const isRunning = parsed?.kind === 'Running'
  const isDisabled = parsed?.kind === 'Disabled'

  // ── Handlers ──

  const handleTrigger = async () => {
    if (triggering || isRunning) return
    setTriggering(true)
    try {
      const report = await daydreamTrigger()
      toast.success(
        `巩固完成：修剪 ${report.prune.pruned} · 合并 ${report.merge.merged} · 刷新 ${report.refresh.refreshed}`,
      )
      void refetchStatus()
      void refetchHistory()
    } catch (e) {
      toast.error(`巩固失败: ${e}`)
    } finally {
      setTriggering(false)
      void refetchStatus()
    }
  }

  const handleToggleEnabled = async () => {
    if (!config) return
    try {
      await updateConfig({ ...config, enabled: !config.enabled })
      void refetchStatus()
      toast.success(config.enabled ? 'DayDream 已禁用' : 'DayDream 已启用')
    } catch (e) {
      toast.error(`切换失败: ${e}`)
    }
  }

  const handleStrategyChange = async (strategy: ConsolidationStrategy) => {
    if (!config) return
    try {
      await updateConfig({ ...config, strategy })
    } catch (e) {
      toast.error(`更新策略失败: ${e}`)
    }
  }

  const handleIdleMinutesChange = async (minutes: number) => {
    if (!config) return
    try {
      await updateConfig({ ...config, idle_trigger_minutes: minutes })
    } catch (e) {
      toast.error(`更新阈值失败: ${e}`)
    }
  }

  const handleSessionEndTriggerChange = async () => {
    if (!config) return
    try {
      await updateConfig({ ...config, session_end_trigger: !config.session_end_trigger })
    } catch (e) {
      toast.error(`更新失败: ${e}`)
    }
  }

  const handleMaxEntriesChange = async (max: number) => {
    if (!config) return
    try {
      await updateConfig({ ...config, max_entries_per_cycle: max })
    } catch (e) {
      toast.error(`更新失败: ${e}`)
    }
  }

  // ── Error display ──

  const combinedError = statusError || configError || historyError

  // ── Render ──

  return (
    <div className="flex flex-col gap-3">
      {/* ── Status & Quick Actions ── */}
      <div className="rounded-xl border border-border/60 bg-card/70 p-4">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <Moon className="h-4.5 w-4.5 text-indigo-500" />
            <div>
              <h3 className="text-[13px] font-semibold text-foreground">Day Dream 记忆巩固</h3>
              <div className="mt-0.5 flex items-center gap-2 text-[12px] text-muted-foreground">
                {statusLoading ? (
                  <span>加载中...</span>
                ) : parsed ? (
                  <>
                    <StatusDot kind={parsed.kind} />
                    <span>{statusLabel(parsed.kind, parsed.finishedAt)}</span>
                  </>
                ) : (
                  <span className="text-muted-foreground/60">未连接</span>
                )}
              </div>
            </div>
          </div>

          <div className="flex items-center gap-2">
            {/* Enable / Disable toggle */}
            {config && (
              <div className="flex items-center gap-2">
                <span className="text-[11px] text-muted-foreground">
                  {config.enabled ? '已启用' : '已禁用'}
                </span>
                <Switch
                  checked={config.enabled}
                  onCheckedChange={handleToggleEnabled}
                  disabled={configLoading}
                />
              </div>
            )}

            {/* Manual trigger */}
            <Button
              type="button"
              size="sm"
              variant="outline"
              disabled={triggering || isRunning || isDisabled}
              onClick={handleTrigger}
              className="h-8 gap-1.5 px-3 text-[12px]"
            >
              {triggering || isRunning ? (
                <RefreshCw className="h-3.5 w-3.5 animate-spin" />
              ) : (
                <Play className="h-3.5 w-3.5" />
              )}
              {triggering ? '巩固中...' : isRunning ? '运行中...' : '手动触发'}
            </Button>
          </div>
        </div>
      </div>

      {/* ── Error banner ── */}
      {combinedError && (
        <div className="flex items-start gap-2.5 rounded-xl border border-red-200/70 bg-red-50 px-4 py-3 text-[11.5px] text-red-700">
          <AlertTriangle className="mt-0.5 h-3.5 w-3.5 shrink-0" />
          {combinedError}
        </div>
      )}

      {/* ── Configuration (collapsible) ── */}
      <div className="rounded-xl border border-border/60 bg-card/70">
        <button
          type="button"
          onClick={() => setConfigOpen(!configOpen)}
          className="flex w-full items-center justify-between px-4 py-3 text-left"
        >
          <div className="flex items-center gap-2">
            <Settings2 className="h-3.5 w-3.5 text-muted-foreground" />
            <span className="text-[12px] font-medium text-foreground">巩固配置</span>
          </div>
          {configOpen ? (
            <ChevronDown className="h-3.5 w-3.5 text-muted-foreground" />
          ) : (
            <ChevronRight className="h-3.5 w-3.5 text-muted-foreground" />
          )}
        </button>

        {configOpen && config && (
          <div className="border-t border-border/40 px-4 pb-4 pt-3">
            <div className="flex flex-col gap-4">
              {/* Idle trigger minutes */}
              <div className="flex items-center justify-between gap-4">
                <div>
                  <div className="text-[12px] font-medium text-foreground/90">空闲触发阈值</div>
                  <div className="text-[11px] text-muted-foreground">用户空闲超过此时间后自动巩固</div>
                </div>
                <div className="flex items-center gap-2">
                  <input
                    type="number"
                    min={1}
                    max={120}
                    value={config.idle_trigger_minutes}
                    onChange={(e) => {
                      const v = parseInt(e.target.value, 10)
                      if (!isNaN(v) && v >= 1 && v <= 120) void handleIdleMinutesChange(v)
                    }}
                    className="w-16 rounded-md border border-border/70 bg-card/80 px-2 py-1.5 text-center text-[12px] outline-none focus:border-primary/40 focus:ring-1 focus:ring-primary/20"
                  />
                  <span className="text-[11px] text-muted-foreground">分钟</span>
                </div>
              </div>

              {/* Strategy */}
              <div>
                <div className="mb-2 text-[12px] font-medium text-foreground/90">巩固策略</div>
                <div className="flex gap-2">
                  {STRATEGY_OPTIONS.map((opt) => (
                    <button
                      key={opt.value}
                      type="button"
                      onClick={() => void handleStrategyChange(opt.value)}
                      className={`flex-1 rounded-lg border px-3 py-2 text-left transition-colors ${
                        config.strategy === opt.value
                          ? 'border-primary/40 bg-primary/10 text-primary'
                          : 'border-border/60 bg-card/40 text-muted-foreground hover:bg-accent hover:text-accent-foreground'
                      }`}
                    >
                      <div className="text-[12px] font-medium">{opt.label}</div>
                      <div className="mt-0.5 text-[10px] opacity-70">{opt.desc}</div>
                    </button>
                  ))}
                </div>
              </div>

              {/* Session end trigger */}
              <div className="flex items-center justify-between gap-4">
                <div>
                  <div className="text-[12px] font-medium text-foreground/90">会话结束触发</div>
                  <div className="text-[11px] text-muted-foreground">对话结束时自动执行巩固</div>
                </div>
                <Switch
                  checked={config.session_end_trigger}
                  onCheckedChange={() => void handleSessionEndTriggerChange()}
                />
              </div>

              {/* Max entries per cycle */}
              <div className="flex items-center justify-between gap-4">
                <div>
                  <div className="text-[12px] font-medium text-foreground/90">每周期处理上限</div>
                  <div className="text-[11px] text-muted-foreground">单次巩固最多处理的记忆条数</div>
                </div>
                <input
                  type="number"
                  min={10}
                  max={1000}
                  value={config.max_entries_per_cycle}
                  onChange={(e) => {
                    const v = parseInt(e.target.value, 10)
                    if (!isNaN(v) && v >= 10 && v <= 1000) void handleMaxEntriesChange(v)
                  }}
                  className="w-20 rounded-md border border-border/70 bg-card/80 px-2 py-1.5 text-center text-[12px] outline-none focus:border-primary/40 focus:ring-1 focus:ring-primary/20"
                />
              </div>
            </div>
          </div>
        )}

        {configOpen && !config && configLoading && (
          <div className="border-t border-border/40 px-4 py-6 text-center text-[12px] text-muted-foreground">
            加载配置中...
          </div>
        )}
      </div>

      {/* ── History (collapsible) ── */}
      <div className="rounded-xl border border-border/60 bg-card/70">
        <button
          type="button"
          onClick={() => {
            const next = !historyOpen
            setHistoryOpen(next)
            if (next) void refetchHistory()
          }}
          className="flex w-full items-center justify-between px-4 py-3 text-left"
        >
          <div className="flex items-center gap-2">
            <History className="h-3.5 w-3.5 text-muted-foreground" />
            <span className="text-[12px] font-medium text-foreground">巩固历史</span>
            {reports.length > 0 && (
              <span className="rounded-full bg-muted/60 px-1.5 py-0.5 text-[10px] text-muted-foreground">
                {reports.length}
              </span>
            )}
          </div>
          {historyOpen ? (
            <ChevronDown className="h-3.5 w-3.5 text-muted-foreground" />
          ) : (
            <ChevronRight className="h-3.5 w-3.5 text-muted-foreground" />
          )}
        </button>

        {historyOpen && (
          <div className="border-t border-border/40 px-4 pb-4 pt-3">
            {historyLoading ? (
              <div className="py-4 text-center text-[12px] text-muted-foreground">
                加载历史中...
              </div>
            ) : reports.length === 0 ? (
              <div className="py-4 text-center text-[12px] text-muted-foreground">
                暂无巩固记录。手动触发或等待自动巩固后，报告会显示在这里。
              </div>
            ) : (
              <div className="flex flex-col gap-2">
                {reports
                  .slice()
                  .reverse()
                  .slice(0, 10)
                  .map((report) => (
                    <ReportCard key={report.cycle_id} report={report} />
                  ))}
                {reports.length > 10 && (
                  <div className="text-center text-[11px] text-muted-foreground">
                    仅显示最近 10 条（共 {reports.length} 条）
                  </div>
                )}
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  )
}
