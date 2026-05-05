/**
 * InsightPanel — 洞察和规则展示面板
 *
 * 按分类分组展示所有程序记忆和洞察，每条规则显示分类标签、
 * 内容、trust_score 进度条、适用场景标签和操作按钮。
 *
 * 用于 MemorySettingsPage 的 "进化" tab。
 */

import { useState, useCallback } from 'react'
import { toast } from 'sonner'
import {
  Lightbulb,
  AlertOctagon,
  Award,
  User,
  Wrench,
  Sparkles,
  RefreshCw,
  Trash2,
  Clock,
  Shield,
  TrendingUp,
} from 'lucide-react'
import { Button } from '@/components/ui/button'
import {
  useEvolutionMemories,
  memoryDelete,
  type MemoryEntryDto,
} from '@/api/memory'
import { cn } from '@/lib/utils'

// ── Category meta (shared with EvolutionTimeline) ───────────────────

interface CategoryMeta {
  label: string
  color: string
  bgColor: string
  icon: typeof Lightbulb
}

const INSIGHT_CATEGORIES: Record<string, CategoryMeta> = {
  HeuristicRule: {
    label: '规则',
    color: 'text-blue-600',
    bgColor: 'bg-blue-500/10',
    icon: Lightbulb,
  },
  AntiPattern: {
    label: '反模式',
    color: 'text-orange-600',
    bgColor: 'bg-orange-500/10',
    icon: AlertOctagon,
  },
  BestPractice: {
    label: '最佳实践',
    color: 'text-emerald-600',
    bgColor: 'bg-emerald-500/10',
    icon: Award,
  },
  UserPreference: {
    label: '用户偏好',
    color: 'text-purple-600',
    bgColor: 'bg-purple-500/10',
    icon: User,
  },
  ToolUsagePattern: {
    label: '工具模式',
    color: 'text-gray-600',
    bgColor: 'bg-gray-500/10',
    icon: Wrench,
  },
}

const DEFAULT_META: CategoryMeta = {
  label: '学习',
  color: 'text-indigo-600',
  bgColor: 'bg-indigo-500/10',
  icon: Sparkles,
}

function detectCategory(entry: MemoryEntryDto): string {
  for (const cat of Object.keys(INSIGHT_CATEGORIES)) {
    if (entry.key.includes(cat) || entry.content.includes(cat)) {
      return cat
    }
  }
  if (entry.category === 'Procedural') return 'BestPractice'
  if (entry.category === 'Reflection') return 'HeuristicRule'
  return 'unknown'
}

function getCategoryMeta(cat: string): CategoryMeta {
  return INSIGHT_CATEGORIES[cat] ?? DEFAULT_META
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

// ── Stat card ───────────────────────────────────────────────────────

function StatCard({
  icon: Icon,
  label,
  value,
  color,
}: {
  icon: typeof Shield
  label: string
  value: string | number
  color: string
}) {
  return (
    <div className="flex items-center gap-2.5 rounded-lg border border-border/50 bg-card/60 px-3 py-2">
      <Icon className={cn('h-3.5 w-3.5', color)} />
      <div>
        <div className="text-[13px] font-semibold tabular-nums text-foreground/85">
          {value}
        </div>
        <div className="text-[10px] text-muted-foreground">{label}</div>
      </div>
    </div>
  )
}

// ── Rule card ───────────────────────────────────────────────────────

function RuleCard({
  entry,
  category,
  onDelete,
  deleting,
}: {
  entry: MemoryEntryDto
  category: string
  onDelete: (key: string) => void
  deleting: boolean
}) {
  const meta = getCategoryMeta(category)
  const Icon = meta.icon
  const trustPct = Math.round(entry.trust_score * 100)

  return (
    <div className="group rounded-xl border border-border/60 bg-card/70 px-3.5 py-3 transition-colors hover:bg-accent/30">
      <div className="flex items-start gap-3">
        <div className="min-w-0 flex-1">
          {/* Category badge + timestamp */}
          <div className="flex items-center gap-2">
            <span
              className={cn(
                'inline-flex items-center gap-1 rounded-md px-1.5 py-0.5 text-[10px] font-medium',
                meta.bgColor,
                meta.color,
              )}
            >
              <Icon className="h-2.5 w-2.5" />
              {meta.label}
            </span>
            <span className="text-[10px] text-muted-foreground">
              {entry.category === 'Procedural' ? '程序记忆' : '反省洞察'}
            </span>
            <span className="text-[10px] text-muted-foreground">·</span>
            <span className="flex items-center gap-0.5 text-[10px] text-muted-foreground">
              <Clock className="h-2.5 w-2.5" />
              {formatRelative(entry.updated_at)}
            </span>
          </div>

          {/* Content */}
          <div className="mt-1.5 text-[12px] leading-relaxed text-foreground/85">
            {entry.content}
          </div>

          {/* Trust score bar */}
          <div className="mt-2 flex items-center gap-2">
            <span className="text-[10px] text-muted-foreground">置信度</span>
            <div className="h-1.5 w-24 overflow-hidden rounded-full bg-muted">
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
            <span className="font-mono text-[10px] tabular-nums text-muted-foreground">
              {trustPct}%
            </span>
            <span className="text-[10px] text-muted-foreground">·</span>
            <span className="text-[10px] text-muted-foreground">
              访问 ×{entry.access_count}
            </span>
          </div>
        </div>

        {/* Actions */}
        <div className="flex shrink-0 flex-col gap-1 opacity-0 transition-opacity group-hover:opacity-100">
          <button
            type="button"
            disabled={deleting}
            onClick={() => onDelete(entry.key)}
            className={cn(
              'flex h-6 items-center gap-1 rounded-md border border-border px-2 text-[10px] font-medium',
              'text-muted-foreground transition-colors',
              'hover:border-red-300/60 hover:bg-red-50/60 hover:text-red-700',
              'disabled:cursor-not-allowed disabled:opacity-40',
            )}
            title="删除此规则"
          >
            <Trash2 className="h-2.5 w-2.5" />
            删除
          </button>
        </div>
      </div>
    </div>
  )
}

// ── Main component ──────────────────────────────────────────────────

export function InsightPanel() {
  const { entries, loading, error, refetch } =
    useEvolutionMemories()
  const [refreshing, setRefreshing] = useState(false)
  const [deletingKeys, setDeletingKeys] = useState<Set<string>>(new Set())
  const [filterCategory, setFilterCategory] = useState<string | null>(null)

  const handleRefresh = async () => {
    setRefreshing(true)
    try {
      await refetch()
    } finally {
      setRefreshing(false)
    }
  }

  const handleDelete = useCallback(
    async (key: string) => {
      setDeletingKeys((prev) => new Set(prev).add(key))
      try {
        await memoryDelete(key)
        toast.success('已删除规则')
        void refetch()
      } catch (e) {
        toast.error(`删除失败: ${e}`)
      } finally {
        setDeletingKeys((prev) => {
          const next = new Set(prev)
          next.delete(key)
          return next
        })
      }
    },
    [refetch],
  )

  // Group entries by detected category
  const categorized = entries.reduce<Record<string, MemoryEntryDto[]>>(
    (acc, entry) => {
      const cat = detectCategory(entry)
      if (!acc[cat]) acc[cat] = []
      acc[cat].push(entry)
      return acc
    },
    {},
  )

  const activeCount = entries.filter((e) => e.trust_score > 0.2).length
  const avgConfidence =
    entries.length > 0
      ? Math.round(
          (entries.reduce((sum, e) => sum + e.trust_score, 0) / entries.length) *
            100,
        )
      : 0

  const latestUpdate =
    entries.length > 0 ? formatRelative(entries[0].updated_at) : '—'

  const filteredEntries = filterCategory
    ? entries.filter((e) => detectCategory(e) === filterCategory)
    : entries

  const categoryKeys = Object.keys(categorized).sort()

  return (
    <div className="flex flex-col gap-3">
      {/* Top stats */}
      <div className="grid grid-cols-3 gap-2">
        <StatCard
          icon={Shield}
          label="活跃规则"
          value={activeCount}
          color="text-emerald-600"
        />
        <StatCard
          icon={TrendingUp}
          label="平均置信度"
          value={`${avgConfidence}%`}
          color="text-blue-600"
        />
        <StatCard
          icon={Clock}
          label="最近学习"
          value={latestUpdate}
          color="text-purple-600"
        />
      </div>

      {/* Header with filter */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <h3 className="text-[13px] font-semibold text-foreground">
            已学习的规则
          </h3>
          <span className="rounded-full bg-muted/60 px-1.5 py-0.5 text-[10px] text-muted-foreground">
            {entries.length}
          </span>
        </div>
        <Button
          type="button"
          size="sm"
          variant="outline"
          disabled={refreshing}
          onClick={handleRefresh}
          className="h-7 gap-1.5 px-2 text-[11px]"
        >
          <RefreshCw
            className={cn('h-3 w-3', refreshing && 'animate-spin')}
          />
          刷新
        </Button>
      </div>

      {/* Category filter pills */}
      {categoryKeys.length > 1 && (
        <div className="flex flex-wrap items-center gap-1.5">
          <button
            type="button"
            onClick={() => setFilterCategory(null)}
            className={cn(
              'rounded-md px-2 py-1 text-[10.5px] font-medium transition-colors',
              filterCategory === null
                ? 'bg-primary/10 text-primary'
                : 'bg-muted/50 text-muted-foreground hover:bg-muted',
            )}
          >
            全部 ({entries.length})
          </button>
          {categoryKeys.map((cat) => {
            const meta = getCategoryMeta(cat)
            const count = categorized[cat].length
            return (
              <button
                key={cat}
                type="button"
                onClick={() =>
                  setFilterCategory(filterCategory === cat ? null : cat)
                }
                className={cn(
                  'rounded-md px-2 py-1 text-[10.5px] font-medium transition-colors',
                  filterCategory === cat
                    ? cn(meta.bgColor, meta.color)
                    : 'bg-muted/50 text-muted-foreground hover:bg-muted',
                )}
              >
                {meta.label} ({count})
              </button>
            )
          })}
        </div>
      )}

      {/* Error */}
      {error && (
        <div className="flex items-start gap-2.5 rounded-xl border border-red-200/70 bg-red-50 px-4 py-3 text-[11.5px] text-red-700">
          <AlertOctagon className="mt-0.5 h-3.5 w-3.5 shrink-0" />
          {error}
        </div>
      )}

      {/* Content */}
      {loading ? (
        <div className="py-8 text-center text-[12px] text-muted-foreground">
          加载规则中...
        </div>
      ) : entries.length === 0 ? (
        <div className="rounded-xl border border-dashed border-border bg-muted/30 px-4 py-8 text-center">
          <Sparkles className="mx-auto mb-2 h-5 w-5 text-indigo-500/60" />
          <div className="text-[12px] text-muted-foreground">
            暂无学习到的规则
          </div>
          <div className="mt-1 text-[11px] text-muted-foreground/70">
            Agent 通过自我反省和 DayDream 巩固来提炼行为规则，完成更多任务后规则会出现在这里。
          </div>
        </div>
      ) : (
        <div className="flex flex-col gap-2">
          {filteredEntries.map((entry) => (
            <RuleCard
              key={entry.key}
              entry={entry}
              category={detectCategory(entry)}
              onDelete={handleDelete}
              deleting={deletingKeys.has(entry.key)}
            />
          ))}
        </div>
      )}
    </div>
  )
}
