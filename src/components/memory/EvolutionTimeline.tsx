/**
 * EvolutionTimeline — Agent 进化历程时间线
 *
 * 按时间线展示 Agent 的学习和进化历史，每个节点代表一次学习事件
 * （洞察提取、程序记忆创建、DayDream 巩固等）。
 * 节点类型用不同颜色/图标区分。
 *
 * 用于 MemorySettingsPage 的 "进化" tab。
 */

import { useState } from 'react'
import {
  ChevronDown,
  ChevronRight,
  Lightbulb,
  AlertOctagon,
  Award,
  User,
  Wrench,
  Sparkles,
  RefreshCw,
  Clock,
} from 'lucide-react'
import { Button } from '@/components/ui/button'
import {
  useEvolutionMemories,
  type MemoryEntryDto,
} from '@/api/memory'
import { cn } from '@/lib/utils'

// ── Category display mapping ────────────────────────────────────────

interface CategoryMeta {
  label: string
  color: string
  bgColor: string
  borderColor: string
  icon: typeof Lightbulb
}

const INSIGHT_CATEGORIES: Record<string, CategoryMeta> = {
  HeuristicRule: {
    label: '规则',
    color: 'text-blue-600',
    bgColor: 'bg-blue-500/10',
    borderColor: 'border-blue-300/50',
    icon: Lightbulb,
  },
  AntiPattern: {
    label: '反模式',
    color: 'text-orange-600',
    bgColor: 'bg-orange-500/10',
    borderColor: 'border-orange-300/50',
    icon: AlertOctagon,
  },
  BestPractice: {
    label: '最佳实践',
    color: 'text-emerald-600',
    bgColor: 'bg-emerald-500/10',
    borderColor: 'border-emerald-300/50',
    icon: Award,
  },
  UserPreference: {
    label: '用户偏好',
    color: 'text-purple-600',
    bgColor: 'bg-purple-500/10',
    borderColor: 'border-purple-300/50',
    icon: User,
  },
  ToolUsagePattern: {
    label: '工具模式',
    color: 'text-gray-600',
    bgColor: 'bg-gray-500/10',
    borderColor: 'border-gray-300/50',
    icon: Wrench,
  },
}

const DEFAULT_CATEGORY_META: CategoryMeta = {
  label: '学习',
  color: 'text-indigo-600',
  bgColor: 'bg-indigo-500/10',
  borderColor: 'border-indigo-300/50',
  icon: Sparkles,
}

function getCategoryMeta(entry: MemoryEntryDto): CategoryMeta {
  // Try to extract insight category from the key or content
  // Keys may follow patterns like "insight:HeuristicRule:..." or
  // content may contain category hints
  for (const [cat, meta] of Object.entries(INSIGHT_CATEGORIES)) {
    if (entry.key.includes(cat) || entry.content.includes(cat)) {
      return meta
    }
  }
  // Fallback based on memory category
  if (entry.category === 'Procedural') {
    return INSIGHT_CATEGORIES.BestPractice
  }
  if (entry.category === 'Reflection') {
    return INSIGHT_CATEGORIES.HeuristicRule
  }
  return DEFAULT_CATEGORY_META
}

function formatRelative(rfc3339: string): string {
  try {
    const ts = new Date(rfc3339).getTime()
    const diffSec = Math.max(0, (Date.now() - ts) / 1000)
    if (diffSec < 60) return '刚刚'
    if (diffSec < 3600) return `${Math.floor(diffSec / 60)} 分钟前`
    if (diffSec < 86400) return `${Math.floor(diffSec / 3600)} 小时前`
    if (diffSec < 2592000) return `${Math.floor(diffSec / 86400)} 天前`
    return new Date(rfc3339).toLocaleDateString('zh-CN', {
      month: '2-digit',
      day: '2-digit',
    })
  } catch {
    return rfc3339
  }
}

function formatTimestamp(rfc3339: string): string {
  try {
    return new Date(rfc3339).toLocaleString('zh-CN', {
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
    })
  } catch {
    return rfc3339
  }
}

function contentSummary(content: string, maxLen = 120): string {
  const trimmed = content.trim()
  if (trimmed.length <= maxLen) return trimmed
  return trimmed.slice(0, maxLen) + '…'
}

// ── Timeline node component ─────────────────────────────────────────

function TimelineNode({ entry }: { entry: MemoryEntryDto }) {
  const [expanded, setExpanded] = useState(false)
  const meta = getCategoryMeta(entry)
  const Icon = meta.icon
  const trustPct = Math.round(entry.trust_score * 100)

  return (
    <div className="relative flex gap-3 pb-4">
      {/* Vertical timeline line */}
      <div className="flex flex-col items-center">
        <div
          className={cn(
            'flex h-7 w-7 shrink-0 items-center justify-center rounded-full border',
            meta.bgColor,
            meta.borderColor,
          )}
        >
          <Icon className={cn('h-3.5 w-3.5', meta.color)} />
        </div>
        <div className="mt-1 flex-1 w-px bg-border/60" />
      </div>

      {/* Content */}
      <div className="min-w-0 flex-1 pt-0.5">
        <button
          type="button"
          onClick={() => setExpanded(!expanded)}
          className="flex w-full items-start justify-between gap-2 text-left"
        >
          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-2">
              <span
                className={cn(
                  'inline-flex items-center rounded-md px-1.5 py-0.5 text-[10px] font-medium',
                  meta.bgColor,
                  meta.color,
                )}
              >
                {meta.label}
              </span>
              <span className="text-[10.5px] text-muted-foreground">
                {entry.category === 'Procedural' ? '程序记忆' : '反省洞察'}
              </span>
              <span className="text-[10.5px] text-muted-foreground">·</span>
              <span className="text-[10.5px] text-muted-foreground">
                {formatRelative(entry.updated_at)}
              </span>
            </div>
            <div className="mt-1 text-[12px] leading-relaxed text-foreground/85">
              {expanded ? entry.content : contentSummary(entry.content)}
            </div>
          </div>
          <div className="shrink-0 pt-0.5">
            {expanded ? (
              <ChevronDown className="h-3.5 w-3.5 text-muted-foreground" />
            ) : (
              <ChevronRight className="h-3.5 w-3.5 text-muted-foreground" />
            )}
          </div>
        </button>

        {expanded && (
          <div className="mt-2 rounded-lg border border-border/50 bg-muted/30 px-3 py-2.5">
            <div className="flex flex-wrap items-center gap-3 text-[11px] text-muted-foreground">
              <span className="flex items-center gap-1">
                <Clock className="h-3 w-3" />
                创建 {formatTimestamp(entry.created_at)}
              </span>
              <span>·</span>
              <span>更新 {formatTimestamp(entry.updated_at)}</span>
              <span>·</span>
              <span>访问 ×{entry.access_count}</span>
            </div>
            {/* Trust score bar */}
            <div className="mt-2 flex items-center gap-2">
              <span className="text-[10.5px] text-muted-foreground">置信度</span>
              <div className="h-1.5 flex-1 overflow-hidden rounded-full bg-muted">
                <div
                  className={cn(
                    'h-full rounded-full transition-all duration-300',
                    trustPct > 50
                      ? 'bg-emerald-500/70'
                      : trustPct > 20
                        ? 'bg-amber-500/70'
                        : 'bg-red-500/70',
                  )}
                  style={{ width: `${trustPct}%` }}
                />
              </div>
              <span className="font-mono text-[10.5px] tabular-nums text-muted-foreground">
                {trustPct}%
              </span>
            </div>
            {/* Key */}
            <div className="mt-1.5 text-[10px] font-mono text-muted-foreground/70 break-all">
              key: {entry.key}
            </div>
          </div>
        )}
      </div>
    </div>
  )
}

// ── Main component ──────────────────────────────────────────────────

const PAGE_SIZE = 20

export function EvolutionTimeline() {
  const { entries, procedureCount, insightCount, loading, error, refetch } =
    useEvolutionMemories()
  const [refreshing, setRefreshing] = useState(false)
  const [showCount, setShowCount] = useState(PAGE_SIZE)

  const handleRefresh = async () => {
    setRefreshing(true)
    try {
      await refetch()
    } finally {
      setRefreshing(false)
    }
  }

  const activeCount = entries.filter((e) => e.trust_score > 0.2).length
  const visibleEntries = entries.slice(0, showCount)

  return (
    <div className="flex flex-col gap-3">
      {/* Header stats */}
      <div className="rounded-xl border border-border/60 bg-card/70 p-4">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <Sparkles className="h-4.5 w-4.5 text-indigo-500" />
            <div>
              <h3 className="text-[13px] font-semibold text-foreground">
                进化历程
              </h3>
              <div className="mt-0.5 text-[12px] text-muted-foreground">
                {loading ? (
                  '加载中...'
                ) : (
                  <>
                    共学到{' '}
                    <span className="font-semibold text-foreground/80">
                      {entries.length}
                    </span>{' '}
                    条规则，其中{' '}
                    <span className="font-semibold text-emerald-600">
                      {activeCount}
                    </span>{' '}
                    条活跃
                  </>
                )}
              </div>
            </div>
          </div>

          <Button
            type="button"
            size="sm"
            variant="outline"
            disabled={refreshing}
            onClick={handleRefresh}
            className="h-8 gap-1.5 px-3 text-[12px]"
          >
            <RefreshCw
              className={cn('h-3.5 w-3.5', refreshing && 'animate-spin')}
            />
            刷新
          </Button>
        </div>

        {/* Category breakdown */}
        {!loading && entries.length > 0 && (
          <div className="mt-3 flex items-center gap-3 text-[11px]">
            <span className="flex items-center gap-1.5 rounded-md bg-emerald-500/10 px-2 py-1 text-emerald-700">
              <Award className="h-3 w-3" />
              程序记忆 {procedureCount}
            </span>
            <span className="flex items-center gap-1.5 rounded-md bg-blue-500/10 px-2 py-1 text-blue-700">
              <Lightbulb className="h-3 w-3" />
              反省洞察 {insightCount}
            </span>
          </div>
        )}
      </div>

      {/* Error */}
      {error && (
        <div className="flex items-start gap-2.5 rounded-xl border border-red-200/70 bg-red-50 px-4 py-3 text-[11.5px] text-red-700">
          <AlertOctagon className="mt-0.5 h-3.5 w-3.5 shrink-0" />
          {error}
        </div>
      )}

      {/* Timeline */}
      {loading ? (
        <div className="py-8 text-center text-[12px] text-muted-foreground">
          加载进化历程中...
        </div>
      ) : entries.length === 0 ? (
        <div className="rounded-xl border border-dashed border-border bg-muted/30 px-4 py-8 text-center">
          <Sparkles className="mx-auto mb-2 h-5 w-5 text-indigo-500/60" />
          <div className="text-[12px] text-muted-foreground">
            暂无进化记录
          </div>
          <div className="mt-1 text-[11px] text-muted-foreground/70">
            Agent 在完成任务后会自我反省并提炼规则，这些学习成果会显示在这里。
          </div>
        </div>
      ) : (
        <div className="rounded-xl border border-border/60 bg-card/70 px-4 pt-4 pb-2">
          {visibleEntries.map((entry) => (
            <TimelineNode key={entry.key} entry={entry} />
          ))}
          {entries.length > showCount && (
            <div className="pb-2 text-center">
              <button
                type="button"
                onClick={() => setShowCount((c) => c + PAGE_SIZE)}
                className="text-[11.5px] text-primary hover:underline"
              >
                加载更多（还有 {entries.length - showCount} 条）
              </button>
            </div>
          )}
        </div>
      )}
    </div>
  )
}
