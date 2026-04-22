/**
 * CompiledMemoryViewer — modal that renders the four compiled-memory
 * sections (today/week/longterm/facts) plus the assembled memory.md.
 *
 * Phase 8B.10 / T-UI-2.  Trigger sources:
 *   - "查看 AI 记忆" button in MemorySettingsPage.
 *   - "记忆已加载" badge in ContextBar (8A.12 placeholder, now wired).
 *
 * Behaviour:
 *   - Loads compiled snapshot via `memoryCompiledRead` on every open.
 *   - "重新编译" calls `memoryCompileNow` then refreshes.
 *   - "清空编译" is a two-step armed confirm (auto-disarms after 6s).
 *   - Closes on backdrop click and on `Escape` key.
 */

import { useCallback, useEffect, useState } from 'react'
import { toast } from 'sonner'
import { Loader2, RefreshCw, Trash2, X } from 'lucide-react'
import { CompiledSectionPanel } from './CompiledSectionPanel'
import { useRuntimeProjectionSelector } from '@/runtime-projection'
import {
  memoryCompileNow,
  memoryCompiledClear,
  memoryCompiledRead,
  type CompiledMemoryDto,
} from '@/lib/tauri'

export interface CompiledMemoryViewerProps {
  /** Whether the modal is currently visible. */
  open: boolean
  /** Caller-supplied close handler (backdrop / X / ESC). */
  onClose: () => void
  /**
   * Compile scope passed through to the backend; defaults to `'global'`
   * which matches how the system-prompt assembler reads memory.md.
   */
  scope?: 'current' | 'all' | 'project' | 'global'
}

type Tab = 'memory' | 'today' | 'week' | 'longterm' | 'facts'

const TAB_LABELS: Record<Tab, string> = {
  memory: '汇总 memory.md',
  today: '今天',
  week: '过去 7 天',
  longterm: '长期',
  facts: '关键事实',
}

/**
 * Modal viewer for the compiled long-term memory (memory.md) injected into
 * the system prompt.  Wired from MemorySettingsPage and ContextBar.
 */
export function CompiledMemoryViewer({
  open,
  onClose,
  scope = 'global',
}: CompiledMemoryViewerProps) {
  const [dto, setDto] = useState<CompiledMemoryDto | null>(null)
  const [loading, setLoading] = useState(false)
  const [tab, setTab] = useState<Tab>('memory')
  const [recompiling, setRecompiling] = useState(false)
  const [clearArmed, setClearArmed] = useState(false)
  const [clearing, setClearing] = useState(false)

  const refresh = useCallback(async () => {
    setLoading(true)
    try {
      const m = await memoryCompiledRead(scope)
      setDto(m)
    } catch (e) {
      toast.error('读取编译记忆失败', { description: String(e) })
    } finally {
      setLoading(false)
    }
  }, [scope])

  // Memory Audit P1 #5 — only react to invalidations whose scope hint
  // matches "compiled" or "all"; refetch otherwise gives noisy load
  // (e.g. unrelated entry deletes shouldn't re-read all 4 markdowns).
  const invalidationVersion = useRuntimeProjectionSelector(
    (s) => s.memory.invalidationVersion,
  )
  const lastInvalidationScope = useRuntimeProjectionSelector(
    (s) => s.memory.lastInvalidationScope,
  )
  const compiledInvalidationKey =
    lastInvalidationScope === 'compiled' || lastInvalidationScope === 'all'
      ? invalidationVersion
      : 0

  useEffect(() => {
    if (open) void refresh()
  }, [open, refresh, compiledInvalidationKey])

  // Close on Escape.
  useEffect(() => {
    if (!open) return
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose()
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [open, onClose])

  // Auto-disarm clear button after 6s.
  useEffect(() => {
    if (!clearArmed) return
    const t = window.setTimeout(() => setClearArmed(false), 6000)
    return () => window.clearTimeout(t)
  }, [clearArmed])

  const handleRecompile = async () => {
    setRecompiling(true)
    try {
      const r = await memoryCompileNow(scope)
      const compiled = [r.today, r.week, r.longterm, r.facts].filter(
        (x) => x === 'compiled',
      ).length
      toast.success(`编译完成 (${compiled}/4 重编译，${r.elapsed_ms}ms)`, {
        description: `assembled: ${r.assembled ? '✅' : '❌'}`,
      })
      await refresh()
    } catch (e) {
      toast.error('编译失败', { description: String(e) })
    } finally {
      setRecompiling(false)
    }
  }

  const handleClear = async () => {
    if (!clearArmed) {
      setClearArmed(true)
      return
    }
    setClearing(true)
    try {
      await memoryCompiledClear(scope)
      toast.success('已清空所有编译记忆')
      setClearArmed(false)
      await refresh()
    } catch (e) {
      toast.error('清空失败', { description: String(e) })
    } finally {
      setClearing(false)
    }
  }

  if (!open) return null

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4"
      onClick={onClose}
      role="dialog"
      aria-modal="true"
      aria-label="AI 长期记忆查看器"
    >
      <div
        className="flex h-[80vh] w-full max-w-5xl flex-col rounded-2xl bg-background shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <header className="flex items-center justify-between border-b border-black/[0.06] px-5 py-3 dark:border-white/[0.08]">
          <div>
            <h2 className="text-base font-semibold">📚 AI 记忆 — memory.md</h2>
            <p className="text-[11px] text-muted-foreground">
              系统提示注入的全部长期记忆 (scope: {scope})
            </p>
          </div>
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={() => void handleRecompile()}
              disabled={recompiling || loading}
              className="flex items-center gap-1.5 rounded-lg bg-blue-600 px-3 py-1.5 text-[12px] text-white shadow-sm transition-all hover:bg-blue-700 disabled:opacity-50"
            >
              {recompiling ? (
                <>
                  <Loader2 className="h-3.5 w-3.5 animate-spin" /> 编译中…
                </>
              ) : (
                <>
                  <RefreshCw className="h-3.5 w-3.5" /> 重新编译
                </>
              )}
            </button>
            <button
              type="button"
              onClick={() => void handleClear()}
              disabled={clearing || loading}
              className={`flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-[12px] shadow-sm transition-all disabled:opacity-50 ${
                clearArmed
                  ? 'bg-red-600 text-white hover:bg-red-700'
                  : 'border border-red-300 text-red-700 hover:bg-red-50 dark:border-red-700/60 dark:text-red-300 dark:hover:bg-red-950/30'
              }`}
            >
              <Trash2 className={`h-3.5 w-3.5 ${clearing ? 'animate-pulse' : ''}`} />
              {clearing ? '清空中…' : clearArmed ? '确认清空编译' : '清空编译'}
            </button>
            <button
              type="button"
              onClick={onClose}
              aria-label="关闭"
              className="rounded-lg p-1.5 text-muted-foreground transition-colors hover:bg-black/[0.04] dark:hover:bg-white/[0.06]"
            >
              <X className="h-4 w-4" />
            </button>
          </div>
        </header>

        {/* Body */}
        <div className="grid flex-1 grid-cols-[180px_1fr] overflow-hidden">
          {/* Tab nav */}
          <nav
            className="flex flex-col gap-1 border-r border-black/[0.06] p-2 dark:border-white/[0.08]"
            role="tablist"
            aria-label="memory.md sections"
          >
            {(Object.keys(TAB_LABELS) as Tab[]).map((t) => {
              const charCount =
                dto == null ? null : t === 'memory' ? dto.memory_md.length : dto[t].chars
              return (
                <button
                  key={t}
                  type="button"
                  role="tab"
                  aria-selected={tab === t}
                  onClick={() => setTab(t)}
                  className={`rounded-lg px-3 py-2 text-left text-[12px] transition-all ${
                    tab === t
                      ? 'bg-blue-100 font-medium text-blue-900 dark:bg-blue-950/40 dark:text-blue-200'
                      : 'text-muted-foreground hover:bg-black/[0.03] dark:hover:bg-white/[0.05]'
                  }`}
                >
                  {TAB_LABELS[t]}
                  {charCount !== null && (
                    <span className="ml-2 font-mono text-[10px] opacity-60">{charCount}</span>
                  )}
                </button>
              )
            })}
          </nav>

          {/* Content */}
          <main className="overflow-hidden p-5">
            {loading || !dto ? (
              <div className="flex h-full items-center justify-center text-[12px] text-muted-foreground">
                <Loader2 className="mr-2 h-4 w-4 animate-spin" /> 加载中…
              </div>
            ) : tab === 'memory' ? (
              <CompiledSectionPanel
                title="memory.md (assemble 输出 — 注入到 system prompt)"
                section={{
                  content: '',
                  last_compiled_at: null,
                  chars: dto.memory_md.length,
                }}
                rawContent={dto.memory_md}
              />
            ) : (
              <CompiledSectionPanel title={TAB_LABELS[tab]} section={dto[tab]} />
            )}
          </main>
        </div>
      </div>
    </div>
  )
}
