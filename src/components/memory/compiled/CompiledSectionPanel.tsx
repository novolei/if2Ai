/**
 * CompiledSectionPanel — single-section renderer for CompiledMemoryViewer.
 *
 * Displays one of memory.md / facts / today / week / longterm with:
 *   - section title + chars count + last_compiled_at relative badge
 *   - ReactMarkdown body (with remark-gfm for tables / strikethrough)
 *   - empty-state placeholder when content is blank
 *
 * Phase 8B.10 / T-UI-2.
 */

import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import { Clock } from 'lucide-react'
import type { CompiledSection } from '@/lib/tauri'

export interface CompiledSectionPanelProps {
  /** Display title shown in the panel header. */
  title: string
  /** Section payload from `memoryCompiledRead` (or a synthetic shell when overriding). */
  section: CompiledSection
  /**
   * Optional override.  When set the panel renders this string instead of
   * `section.content` and hides the per-section "last compiled" badge — used
   * for the assembled `memory.md` tab where the four section mtimes do not
   * correspond to a single compile event.
   */
  rawContent?: string
}

function formatRelative(iso: string | null): string {
  if (!iso) return '从未编译'
  const t = new Date(iso).getTime()
  const diffSec = Math.floor((Date.now() - t) / 1000)
  if (diffSec < 60) return `${diffSec} 秒前`
  if (diffSec < 3600) return `${Math.floor(diffSec / 60)} 分钟前`
  if (diffSec < 86400) return `${Math.floor(diffSec / 3600)} 小时前`
  return `${Math.floor(diffSec / 86400)} 天前`
}

/**
 * Renders one compiled-memory section: header (title + chars + relative
 * timestamp) plus a scrollable Markdown body.
 */
export function CompiledSectionPanel({ title, section, rawContent }: CompiledSectionPanelProps) {
  const content = rawContent ?? section.content
  const chars = rawContent ? rawContent.length : section.chars

  return (
    <div className="flex h-full flex-col">
      <header className="mb-3 flex items-center justify-between border-b border-black/[0.06] pb-2 dark:border-white/[0.08]">
        <h3 className="text-sm font-semibold">{title}</h3>
        <div className="flex items-center gap-3 text-[10.5px] text-muted-foreground">
          <span className="font-mono tabular-nums">{chars} 字</span>
          {!rawContent && (
            <span className="flex items-center gap-1">
              <Clock className="h-3 w-3" aria-hidden />
              {formatRelative(section.last_compiled_at)}
            </span>
          )}
        </div>
      </header>
      <div className="prose prose-sm max-w-none flex-1 overflow-y-auto dark:prose-invert">
        {content.trim() ? (
          <ReactMarkdown remarkPlugins={[remarkGfm]}>{content}</ReactMarkdown>
        ) : (
          <p className="text-[12px] italic text-muted-foreground">
            (本 section 为空 — 待 RollingSummarizer 写入 session_summaries 后下次编译会自动填充)
          </p>
        )}
      </div>
    </div>
  )
}
