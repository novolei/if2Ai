/**
 * SlashResultCard — 紧凑型单行卡，用来美化 git slash 命令的静态输出。
 *
 * 后端 `executeSlashCommand` 对 `/diff` `/branch` `/worktree` ... 的
 * 响应是固定的对齐文本（首行 = 命令名，缩进行 = `Result XXX value`
 * 对齐表）。本组件解析出主结果一行，单行展示；附加体（diff、列表等）
 * 折叠在下方，需要时点开。
 *
 * 视觉上是一个圆角胶囊：
 *   [ icon · /命令 · 主结果文字 ] [ ⌄ 展开 ] (有 body 时)
 *
 * 失败 / 命令未识别时回落到 `<pre>` 显示原文，保证不丢信息。
 */

import * as React from 'react'
import {
  ChevronDown,
  CircleDot,
  GitBranch,
  GitCommitHorizontal,
  GitPullRequestArrow,
  Hash,
  Layers,
  Loader2,
  Send,
  Terminal,
} from 'lucide-react'
import { cn } from '@/lib/utils'

type Props = {
  slashCommand: string
  content: string
}

/** Lookup: command (without leading "/") → display icon. */
const COMMAND_ICONS: Record<string, React.ComponentType<{ className?: string; strokeWidth?: number }>> = {
  diff: Layers,
  status: Hash,
  branch: GitBranch,
  worktree: GitBranch,
  commit: GitCommitHorizontal,
  'commit-push-pr': Send,
  pr: GitPullRequestArrow,
  issue: CircleDot,
}

interface Parsed {
  /** Top header line, e.g. `"Diff"` `"Branch"` `"Worktree"`. */
  header: string
  /** Single-line summary, e.g. `"clean (no staged or unstaged changes)"`. */
  summary: string
  /** Remaining `Key  value` lines that aren't the primary result. */
  details: Array<{ key: string; value: string }>
  /** Raw body block after the table (diff text, listing, etc.). */
  body: string
}

/**
 * Parse the rigid backend table format produced by
 * `crate::modules::git::slash::*`:
 *
 *     Diff
 *       Result           clean (no staged or unstaged changes)
 *
 * or with extra rows + body:
 *
 *     Branch
 *       Result           listed
 *
 *       <branch list>
 *
 * Returns a structured projection; falls back to a single empty
 * summary when the input doesn't look like the expected table.
 */
function parseSlashOutput(raw: string): Parsed {
  const lines = raw.split('\n')
  const header = lines[0]?.trim() ?? ''
  const tableRows: Array<{ key: string; value: string }> = []
  let bodyStart = -1
  for (let i = 1; i < lines.length; i += 1) {
    const line = lines[i]
    const trimmed = line.trim()
    if (!trimmed) {
      // First blank line after the table marks the start of the body
      // block (if there is one).
      if (tableRows.length > 0 && bodyStart === -1) {
        bodyStart = i + 1
      }
      continue
    }
    // Table rows look like `  Key            value` — leading whitespace
    // + a single key word + ≥2 spaces of padding.
    const match = trimmed.match(/^([A-Za-z][A-Za-z\-_0-9]*)\s{2,}(.+)$/)
    if (match && bodyStart === -1) {
      tableRows.push({ key: match[1], value: match[2].trim() })
    } else {
      // First non-table line → treat the rest as raw body.
      bodyStart = i
      break
    }
  }
  const body = bodyStart > -1 ? lines.slice(bodyStart).join('\n').trimEnd() : ''
  // Promote the first `Result` row to the headline; keep the rest as
  // detail chips so commands like `/branch` (Result + Branch + ...)
  // still surface their secondary fields.
  const primaryIndex = tableRows.findIndex(
    (r) => r.key.toLowerCase() === 'result',
  )
  const primary = primaryIndex >= 0 ? tableRows[primaryIndex].value : ''
  const details =
    primaryIndex >= 0
      ? tableRows.slice(0, primaryIndex).concat(tableRows.slice(primaryIndex + 1))
      : tableRows
  return { header, summary: primary, details, body }
}

export function SlashResultCard({ slashCommand, content }: Props) {
  const [expanded, setExpanded] = React.useState(false)
  const parsed = React.useMemo(() => parseSlashOutput(content), [content])
  const cmdKey = slashCommand.replace(/^\//, '').toLowerCase()
  const Icon = COMMAND_ICONS[cmdKey] ?? Terminal
  const hasMore = parsed.details.length > 0 || parsed.body.length > 0

  // 解析失败 / 完全空 → 给一行 raw 兜底，保证用户不会 "命令运行了但没东西看"
  const hasParsedSummary = Boolean(parsed.summary)
  const fallbackSummary = !hasParsedSummary
    ? content.split('\n').find((l) => l.trim())?.trim() ?? ''
    : ''

  return (
    <div className="w-full max-w-[min(680px,80%)]">
      <button
        type="button"
        onClick={() => hasMore && setExpanded((v) => !v)}
        className={cn(
          'group flex w-full items-center gap-2 rounded-xl border border-black/[0.07] bg-white/85 px-3 py-2 text-left text-[13px] leading-6 text-black/82 backdrop-blur-md transition-colors',
          'shadow-[0_1px_2px_rgba(0,0,0,0.03)] hover:bg-white/95',
          !hasMore && 'cursor-default',
        )}
        aria-expanded={hasMore ? expanded : undefined}
      >
        <span
          className="flex h-5 w-5 shrink-0 items-center justify-center rounded-md bg-black/[0.04] text-black/55"
        >
          <Icon className="h-3.5 w-3.5" strokeWidth={1.75} />
        </span>
        <span className="shrink-0 rounded-md bg-black/[0.04] px-1.5 py-0.5 font-mono text-[11.5px] text-black/55">
          {slashCommand}
        </span>
        <span className="min-w-0 flex-1 truncate text-black/72">
          {hasParsedSummary ? parsed.summary : fallbackSummary || parsed.header || '已执行'}
        </span>
        {hasMore && (
          <ChevronDown
            className={cn(
              'h-3.5 w-3.5 shrink-0 text-black/35 transition-transform',
              expanded && 'rotate-180',
            )}
            strokeWidth={1.75}
          />
        )}
      </button>

      {expanded && hasMore && (
        <div className="mt-1.5 overflow-hidden rounded-xl border border-black/[0.05] bg-white/65 backdrop-blur-md">
          {parsed.details.length > 0 && (
            <ul className="divide-y divide-black/[0.04]">
              {parsed.details.map((row) => (
                <li
                  key={`${row.key}-${row.value}`}
                  className="flex items-baseline gap-3 px-3 py-1.5 text-[12.5px] leading-5"
                >
                  <span className="w-20 shrink-0 text-black/40">{row.key}</span>
                  <span className="min-w-0 flex-1 break-words font-mono text-[12px] text-black/75">
                    {row.value}
                  </span>
                </li>
              ))}
            </ul>
          )}
          {parsed.body && (
            <pre className="m-0 max-h-[320px] overflow-auto whitespace-pre-wrap break-words border-t border-black/[0.04] px-3 py-2 font-mono text-[11.5px] leading-5 text-black/75">
              {parsed.body}
            </pre>
          )}
        </div>
      )}
    </div>
  )
}

/**
 * In-flight skeleton variant.  Not currently rendered by `ChatMessage`
 * (slash IPC is sync from the user's perspective — `executeSlashCommand`
 * resolves before the message is ever appended), but exported so the
 * Workbench can reuse the same visual idiom for asynchronous commands.
 */
export function SlashResultCardSkeleton({ slashCommand }: { slashCommand: string }) {
  return (
    <div className="flex w-full max-w-[min(680px,80%)] items-center gap-2 rounded-xl border border-black/[0.07] bg-white/70 px-3 py-2 text-[13px] leading-6 text-black/45">
      <span className="flex h-5 w-5 shrink-0 items-center justify-center rounded-md bg-black/[0.04]">
        <Loader2 className="h-3.5 w-3.5 animate-spin" />
      </span>
      <span className="shrink-0 rounded-md bg-black/[0.04] px-1.5 py-0.5 font-mono text-[11.5px] text-black/55">
        {slashCommand}
      </span>
      <span className="text-black/45">运行中…</span>
    </div>
  )
}
