/**
 * MemoryCard — 单条记忆卡片组件
 *
 * 显示 key, content, category, created_at, importance 进度条,
 * trust_score 颜色渐变, access_count。
 *
 * Props:
 * - entry: 记忆条目数据
 * - onDelete: 删除回调
 */

import { ArrowDown, Trash2 } from 'lucide-react'
import { cn } from '@/lib/utils'

export interface MemoryEntryDto {
  key: string
  content: string
  category: string
  created_at: string
  updated_at: string
  importance: number
  access_count: number
  trust_score: number
  /** Persisted session scope tag; null = entry not session-scoped. */
  session_id: string | null
  /** Persisted project scope tag; null = entry not project-scoped. */
  project_id: string | null
}

export interface MemoryCardProps {
  entry: MemoryEntryDto
  onDelete: (key: string) => void
  /**
   * Callback for the "降一级" (demote) action.
   *
   * The card itself only knows the entry's *current* tier; the parent (e.g.
   * MemoryBrowser) owns the active project/session context, so the demote
   * button delegates the decision of "where to land" to the parent.  Pass
   * `undefined` to hide the demote affordance entirely.
   */
  onDemote?: (entry: MemoryEntryDto) => void
  /**
   * `true` when the parent has an active project/session pair that lets the
   * demote target be unambiguous.  When `false` the button is rendered but
   * disabled with an explanatory tooltip — keeps the affordance discoverable
   * without firing a "missing context" error toast on click.
   */
  canDemote?: boolean
}

function formatTime(isoString: string): string {
  try {
    const date = new Date(isoString)
    const now = new Date()
    const diffMs = now.getTime() - date.getTime()
    const diffHours = Math.floor(diffMs / (1000 * 60 * 60))
    if (diffHours < 1) return '刚刚'
    if (diffHours < 24) return `${diffHours} 小时前`
    const diffDays = Math.floor(diffHours / 24)
    if (diffDays < 7) return `${diffDays} 天前`
    return date.toLocaleDateString('zh-CN')
  } catch {
    return isoString
  }
}

function getTrustScoreColor(score: number): string {
  // -1.0 (red) → 0.0 (gray) → 1.0 (green)
  if (score > 0.3) return 'text-green-600'
  if (score > 0) return 'text-green-500'
  if (score > -0.3) return 'text-gray-500'
  return 'text-red-500'
}

function getTrustScoreLabel(score: number): string {
  if (score > 0.5) return '可信'
  if (score > 0) return '中性'
  if (score > -0.5) return '存疑'
  return '不可信'
}

/**
 * Derive the visible scope tier of an entry from its persisted tags.
 *
 * Mirrors the SQL precedence in `SqliteMemoryProvider::recall_scoped`:
 * - `session_id != NULL`              → session
 * - `session_id == NULL && project_id != NULL` → project
 * - both NULL                          → global
 */
function getScopeChip(entry: { session_id: string | null; project_id: string | null }) {
  if (entry.session_id) {
    return {
      label: '会话',
      title: `Session ${entry.session_id}`,
      cls: 'bg-violet-50 text-violet-700 border-violet-200',
    }
  }
  if (entry.project_id) {
    return {
      label: '项目',
      title: `Project ${entry.project_id}`,
      cls: 'bg-sky-50 text-sky-700 border-sky-200',
    }
  }
  return {
    label: '全局',
    title: '全局共享记忆（所有项目可见）',
    cls: 'bg-emerald-50 text-emerald-700 border-emerald-200',
  }
}

export function MemoryCard({ entry, onDelete, onDemote, canDemote }: MemoryCardProps) {
  const scope = getScopeChip(entry)
  // Global → project → session: only entries currently above session may
  // be demoted.  We hide the action entirely on session-tier rows so the
  // hover surface stays clean for the common case.
  const showDemote = Boolean(onDemote) && (entry.project_id !== null || entry.session_id === null)
  return (
    <div className="group relative rounded-lg border border-black/5 bg-white/60 px-4 py-3 shadow-sm transition-shadow duration-150 hover:shadow-md">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2">
            <span className="truncate text-[12px] font-medium text-black/60">
              {entry.key}
            </span>
            <span className="shrink-0 rounded-full bg-black/5 px-2 py-0.5 text-[10px] text-black/50">
              {entry.category}
            </span>
            <span
              title={scope.title}
              className={cn(
                'shrink-0 rounded-full border px-2 py-0.5 text-[10px] font-medium',
                scope.cls,
              )}
            >
              {scope.label}
            </span>
          </div>
          <p className="mt-1 line-clamp-3 text-[13px] leading-relaxed text-foreground/80">
            {entry.content}
          </p>
          <div className="mt-2 flex items-center gap-4 text-[11px] text-black/40">
            <span>{formatTime(entry.created_at)}</span>
            <span>访问 {entry.access_count} 次</span>
            <span className={cn('font-medium', getTrustScoreColor(entry.trust_score))}>
              {getTrustScoreLabel(entry.trust_score)} ({entry.trust_score.toFixed(2)})
            </span>
          </div>
          <div className="mt-1.5 h-1 w-full overflow-hidden rounded-full bg-black/5">
            <div
              className="h-full rounded-full bg-primary/40 transition-all duration-300"
              style={{ width: `${Math.round(entry.importance * 100)}%` }}
            />
          </div>
        </div>
        <div className="flex shrink-0 flex-col gap-1">
          {showDemote && (
            <button
              type="button"
              className={cn(
                'shrink-0 rounded p-1 transition-all duration-150 group-hover:opacity-100',
                canDemote
                  ? 'text-amber-500/70 opacity-0 hover:bg-amber-50 hover:text-amber-600'
                  : 'cursor-not-allowed text-black/20 opacity-0',
              )}
              disabled={!canDemote}
              onClick={() => onDemote?.(entry)}
              aria-label="降级记忆"
              title={
                canDemote
                  ? '降一级（将该记忆移到当前项目/会话范围）'
                  : '需要打开一个项目（以及对应会话）才能降级到更小范围'
              }
            >
              <ArrowDown className="h-4 w-4" />
            </button>
          )}
          <button
            type="button"
            className="shrink-0 rounded p-1 text-black/30 opacity-0 transition-all duration-150 hover:bg-red-50 hover:text-red-500 group-hover:opacity-100"
            onClick={() => onDelete(entry.key)}
            aria-label="删除记忆"
          >
            <Trash2 className="h-4 w-4" />
          </button>
        </div>
      </div>
    </div>
  )
}
