import { useCallback, useEffect, useMemo, useState } from 'react'
import {
  Bot,
  Brain,
  Coins,
  FileText,
  Hammer,
  Layers,
  RefreshCw,
  Sparkles,
} from 'lucide-react'
import { toast } from 'sonner'

import { Button } from '@/components/ui/button'
import {
  usageSummary,
  type CallerUsage,
  type UsageSummary,
  type UsageWindow,
} from '@/lib/tauri'
import { cn } from '@/lib/utils'

import { SettingsMetricCard } from '../components/SettingsMetricCard'
import { SettingsSurface } from '../components/SettingsSurface'
import { SettingsToggleRow } from '../components/SettingsToggleRow'
import type { SettingsPageProps } from '../types'

/**
 * Settings ▸ 用量统计 — driven by the durable per-role usage store
 * (`~/.if2ai/usage/usage.sqlite`, populated by `turn_service` for the
 * chat path and by `ChatProviderUtilityLlm.with_caller(...)` for
 * memory summarizer / compiler / utility one-shots).
 *
 * Layout:
 *   - 4 metric cards (今日 / 本周 / 本月 / 全部) for the *active*
 *     window's totals;
 *   - tab switcher to choose the active window;
 *   - by-caller table aligned with the LLM model role meta in
 *     `ModelSettingsPage` (chat / utility / utility_large /
 *     summarizer / compiler), plus an "其他" bucket for unknown
 *     callers so future code never goes invisible.
 */

const WINDOW_ORDER: { key: UsageWindow; label: string }[] = [
  { key: 'today', label: '今日' },
  { key: 'this_week', label: '本周' },
  { key: 'this_month', label: '本月' },
  { key: 'all_time', label: '全部' },
]

interface RoleRow {
  key: string
  label: string
  description: string
  icon: typeof Bot
}

const ROLE_ROWS: RoleRow[] = [
  {
    key: 'chat',
    label: '主对话 (chat)',
    description: '聊天窗口的用户回合',
    icon: Bot,
  },
  {
    key: 'utility',
    label: '轻量工具 (utility)',
    description: '小型一次性提示（问候 / 分类器等）',
    icon: Sparkles,
  },
  {
    key: 'utility_large',
    label: '重量工具 (utility_large)',
    description: '长文本一次性任务',
    icon: FileText,
  },
  {
    key: 'summarizer',
    label: '摘要 (summarizer)',
    description: 'RollingSummarizer 滚动摘要',
    icon: Layers,
  },
  {
    key: 'compiler',
    label: '编译 (compiler)',
    description: '记忆编译 / reflection 等长任务',
    icon: Hammer,
  },
]

function fmtTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(2)}M`
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`
  return n.toString()
}

function fmtCost(usd: number): string {
  if (usd <= 0) return '$0.00'
  if (usd < 0.01) return `$${usd.toFixed(4)}`
  if (usd < 1) return `$${usd.toFixed(3)}`
  return `$${usd.toFixed(2)}`
}

const EMPTY_BY_CALLER: CallerUsage = {
  caller: 'unknown',
  inputTokens: 0,
  outputTokens: 0,
  costUsd: 0,
  turns: 0,
}

export function UsageSettingsPage({ state, actions }: SettingsPageProps) {
  const [activeWindow, setActiveWindow] = useState<UsageWindow>('today')
  const [summaries, setSummaries] = useState<Partial<Record<UsageWindow, UsageSummary>>>({})
  const [busy, setBusy] = useState(false)

  const refresh = useCallback(async () => {
    setBusy(true)
    try {
      const results = await Promise.all(
        WINDOW_ORDER.map(async ({ key }) => [key, await usageSummary(key)] as const),
      )
      setSummaries(Object.fromEntries(results))
    } catch (err) {
      toast.error(`读取用量失败: ${(err as Error).message ?? err}`)
    } finally {
      setBusy(false)
    }
  }, [])

  useEffect(() => {
    void refresh()
  }, [refresh])

  const summaryForWindow = useCallback(
    (key: UsageWindow): UsageSummary | undefined => summaries[key],
    [summaries],
  )

  const summaryByCaller = useCallback(
    (s: UsageSummary | undefined, caller: string): CallerUsage => {
      if (!s) return EMPTY_BY_CALLER
      return s.byCaller.find((c) => c.caller === caller) ?? EMPTY_BY_CALLER
    },
    [],
  )

  // "其他"桶：把 ROLE_ROWS 没列出的 caller 汇总成一行（防御性）。
  const otherRow = useMemo(() => {
    const s = summaryForWindow(activeWindow)
    if (!s) return null
    const known = new Set(ROLE_ROWS.map((r) => r.key))
    const others = s.byCaller.filter((c) => !known.has(c.caller))
    if (others.length === 0) return null
    return others.reduce<CallerUsage>(
      (acc, cur) => ({
        caller: 'other',
        inputTokens: acc.inputTokens + cur.inputTokens,
        outputTokens: acc.outputTokens + cur.outputTokens,
        costUsd: acc.costUsd + cur.costUsd,
        turns: acc.turns + cur.turns,
      }),
      { ...EMPTY_BY_CALLER },
    )
  }, [activeWindow, summaryForWindow])

  return (
    <div className="flex flex-col gap-3">
      {/* ── Metric cards: one per window, showing total tokens + cost ── */}
      <div className="grid gap-2.5 sm:grid-cols-2 xl:grid-cols-4">
        {WINDOW_ORDER.map(({ key, label }) => {
          const s = summaryForWindow(key)
          const total = s?.total
          const totalTokens = (total?.inputTokens ?? 0) + (total?.outputTokens ?? 0)
          const cost = total?.costUsd ?? 0
          const detail = total
            ? `${fmtTokens(total.inputTokens)} 输入 · ${fmtTokens(total.outputTokens)} 输出 · ${fmtCost(cost)} · ${total.turns} 调用`
            : busy
              ? '加载中…'
              : '尚无数据'
          return (
            <SettingsMetricCard
              key={key}
              icon={key === 'all_time' ? Coins : Brain}
              label={label}
              value={fmtTokens(totalTokens)}
              detail={detail}
              className={cn(
                'transition-all',
                activeWindow === key &&
                  'ring-2 ring-jade/40 shadow-[0_4px_20px_rgba(0,0,0,0.08)]',
              )}
            />
          )
        })}
      </div>

      {/* ── By-caller table ── */}
      <SettingsSurface className="px-5 py-4">
        <div className="mb-3 flex items-center justify-between gap-3">
          <div className="flex items-center gap-1.5">
            <div className="text-[10.5px] font-semibold uppercase tracking-widest text-black/30">
              按角色分类
            </div>
            <div className="ml-2 inline-flex rounded-lg bg-black/[0.04] p-0.5">
              {WINDOW_ORDER.map(({ key, label }) => (
                <button
                  key={key}
                  type="button"
                  onClick={() => setActiveWindow(key)}
                  className={cn(
                    'rounded-md px-2.5 py-1 text-[11.5px] font-medium transition-colors',
                    activeWindow === key
                      ? 'bg-white text-foreground shadow-sm'
                      : 'text-muted-foreground hover:text-foreground',
                  )}
                >
                  {label}
                </button>
              ))}
            </div>
          </div>
          <Button
            variant="ghost"
            disabled={busy}
            onClick={() => void refresh()}
            className="window-no-drag h-7 rounded-xl px-2.5 text-[11.5px] text-muted-foreground hover:bg-black/[0.04] hover:text-foreground"
          >
            <RefreshCw className={cn('mr-1.5 h-3.5 w-3.5', busy && 'animate-spin')} />
            刷新
          </Button>
        </div>

        <div className="overflow-hidden rounded-xl border border-black/[0.06]">
          <table className="w-full border-collapse text-[12px]">
            <thead className="bg-black/[0.025] text-[10.5px] uppercase tracking-widest text-black/40">
              <tr>
                <th className="px-3 py-2 text-left font-semibold">角色</th>
                <th className="px-3 py-2 text-right font-semibold">输入</th>
                <th className="px-3 py-2 text-right font-semibold">输出</th>
                <th className="px-3 py-2 text-right font-semibold">调用</th>
                <th className="px-3 py-2 text-right font-semibold">成本</th>
              </tr>
            </thead>
            <tbody>
              {ROLE_ROWS.map((row, i) => {
                const c = summaryByCaller(summaryForWindow(activeWindow), row.key)
                const Icon = row.icon
                return (
                  <tr
                    key={row.key}
                    className={cn(
                      'border-t border-black/[0.04]',
                      i === 0 && 'border-t-0',
                    )}
                  >
                    <td className="px-3 py-2">
                      <div className="flex items-center gap-2">
                        <span className="flex size-7 shrink-0 items-center justify-center rounded-lg bg-jade/10 text-jade">
                          <Icon className="h-3.5 w-3.5" />
                        </span>
                        <div>
                          <div className="font-medium">{row.label}</div>
                          <div className="text-[10.5px] text-muted-foreground">
                            {row.description}
                          </div>
                        </div>
                      </div>
                    </td>
                    <td className="px-3 py-2 text-right tabular-nums">
                      {fmtTokens(c.inputTokens)}
                    </td>
                    <td className="px-3 py-2 text-right tabular-nums">
                      {fmtTokens(c.outputTokens)}
                    </td>
                    <td className="px-3 py-2 text-right tabular-nums">{c.turns}</td>
                    <td className="px-3 py-2 text-right tabular-nums">
                      {fmtCost(c.costUsd)}
                    </td>
                  </tr>
                )
              })}
              {otherRow && (
                <tr className="border-t border-black/[0.04]">
                  <td className="px-3 py-2">
                    <div className="flex items-center gap-2">
                      <span className="flex size-7 shrink-0 items-center justify-center rounded-lg bg-black/[0.05] text-black/40">
                        <Sparkles className="h-3.5 w-3.5" />
                      </span>
                      <div>
                        <div className="font-medium">其他 (other)</div>
                        <div className="text-[10.5px] text-muted-foreground">
                          未归类的调用方
                        </div>
                      </div>
                    </div>
                  </td>
                  <td className="px-3 py-2 text-right tabular-nums">
                    {fmtTokens(otherRow.inputTokens)}
                  </td>
                  <td className="px-3 py-2 text-right tabular-nums">
                    {fmtTokens(otherRow.outputTokens)}
                  </td>
                  <td className="px-3 py-2 text-right tabular-nums">{otherRow.turns}</td>
                  <td className="px-3 py-2 text-right tabular-nums">
                    {fmtCost(otherRow.costUsd)}
                  </td>
                </tr>
              )}
            </tbody>
            <tfoot className="bg-black/[0.02] font-semibold">
              <tr>
                <td className="px-3 py-2">汇总</td>
                <td className="px-3 py-2 text-right tabular-nums">
                  {fmtTokens(summaryForWindow(activeWindow)?.total.inputTokens ?? 0)}
                </td>
                <td className="px-3 py-2 text-right tabular-nums">
                  {fmtTokens(summaryForWindow(activeWindow)?.total.outputTokens ?? 0)}
                </td>
                <td className="px-3 py-2 text-right tabular-nums">
                  {summaryForWindow(activeWindow)?.total.turns ?? 0}
                </td>
                <td className="px-3 py-2 text-right tabular-nums">
                  {fmtCost(summaryForWindow(activeWindow)?.total.costUsd ?? 0)}
                </td>
              </tr>
            </tfoot>
          </table>
        </div>

        <p className="mt-2.5 text-[10.5px] leading-4 text-muted-foreground">
          数据持久化于 <code className="rounded bg-black/[0.04] px-1">~/.if2ai/usage/usage.sqlite</code>，
          按 logical-day（04:00 截断）聚合到日 / 周 / 月。 `chat` 来自 turn_service 主流；
          `summarizer` / `compiler` / `utility` 来自 memory pipeline 的 ChatProviderUtilityLlm 包装层。
        </p>
      </SettingsSurface>

      {/* ── Preferences (kept) ── */}
      <SettingsSurface className="px-5 py-4">
        <div className="mb-3 text-[10.5px] font-semibold uppercase tracking-widest text-black/30">
          统计偏好
        </div>
        <div className="flex flex-col">
          <SettingsToggleRow
            title="自动滚动到最新"
            description="保持消息列表跟随输出进度"
            checked={state.autoScroll}
            onCheckedChange={actions.setAutoScroll}
            inline
          />
          <div className="my-0.5 border-t border-black/[0.05]" />
          <SettingsToggleRow
            title="完成时桌面通知"
            description="长任务完成后弹出系统通知"
            checked={state.notifications}
            onCheckedChange={actions.setNotifications}
            inline
          />
        </div>
      </SettingsSurface>
    </div>
  )
}
