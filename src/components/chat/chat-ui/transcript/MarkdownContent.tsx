/**
 * GF-01 PR-03 — extracted from `src/components/ui/chat-ui.tsx`.
 *
 * Renders a chat message body as markdown using `react-markdown` +
 * `remark-gfm` + `rehype-highlight`. The component additionally:
 *  - normalises ASCII relationship diagrams via the shared
 *    `normalizeAsciiDiagramBlocks` helper (cached per content hash),
 *  - short-circuits the render when the body is a `📋 Skills (n total, m enabled)`
 *    slash report, delegating to {@link SkillsSlashReport},
 *  - delegates `<pre>` blocks to {@link CodeBlock} so users can copy.
 *
 * Render-equivalent move — props/behaviour unchanged from the original
 * inline definition.
 */

import * as React from "react"
import ReactMarkdown from "react-markdown"
import remarkGfm from "remark-gfm"
import rehypeHighlight from "rehype-highlight"
import "highlight.js/styles/github.css"
import { cn } from "@/lib/utils"
import { normalizeAsciiDiagramBlocks } from "@/components/chat/chat-ui/utils/markdownNormalize"
import { CodeBlock } from "@/components/chat/chat-ui/cards/CodeBlock"
import { SkillsSlashReport, parseSkillsSlashReport } from "@/components/chat/chat-ui/cards/SkillsSlashReport"

/** Mirrors the `DensityMode` union in `chat-ui.tsx` (kept local to avoid a cross-file type leak). */
type DensityMode = 'comfortable' | 'compact'

/** Same surface-card design tokens used by `chat-ui.tsx`; kept in sync intentionally. */
const SURFACE_CARD_TOKENS = {
  radius: 'rounded-[6px]',
  border: 'border border-border/50',
  background: 'bg-surface',
  headerBackground: 'bg-surface-raised',
  headerDivider: 'border-b border-border/50',
  headerLabel: 'font-mono text-[10px] tracking-tight text-muted-foreground/45',
} as const

/**
 * Module-level memoization cache for normalised markdown sources, keyed by
 * the parent-supplied `contentHash`. Bounded at 300 entries (FIFO eviction).
 *
 * Lives at module scope (not inside the component) so values survive across
 * remounts and avoid recomputing the ASCII-diagram pass on every render.
 */
const markdownNormalizeCache = new Map<number, { source: string; normalized: string }>()

/**
 * Render a chat-message markdown body. Memoized on
 * `(content, contentHash, densityMode)`.
 */
export const MarkdownContent = React.memo(function MarkdownContent({
  content,
  contentHash,
  densityMode,
}: {
  content: string
  contentHash: number
  densityMode: DensityMode
}) {
  const normalizedContent = React.useMemo(() => {
    const cached = markdownNormalizeCache.get(contentHash)
    if (cached && cached.source === content) {
      return cached.normalized
    }
    const normalized = normalizeAsciiDiagramBlocks(content)
    markdownNormalizeCache.set(contentHash, { source: content, normalized })
    if (markdownNormalizeCache.size > 300) {
      const firstKey = markdownNormalizeCache.keys().next().value
      if (firstKey !== undefined) markdownNormalizeCache.delete(firstKey)
    }
    return normalized
  }, [content, contentHash])
  const skillsReport = React.useMemo(() => parseSkillsSlashReport(content), [content])

  if (skillsReport) {
    return <SkillsSlashReport report={skillsReport} densityMode={densityMode} />
  }

  return (
    <ReactMarkdown
      remarkPlugins={[remarkGfm]}
      rehypePlugins={[rehypeHighlight]}
      components={{
        p: ({ children }) => (
          <p
            className={cn(
              'my-2 whitespace-pre-wrap font-normal text-foreground/88 first:mt-0 last:mb-0',
              densityMode === 'compact' ? 'text-[12px] leading-5.25' : 'text-[13px] leading-5.75'
            )}
          >
            {children}
          </p>
        ),
        h1: ({ children }) => <h1 className="scroll-m-20 text-xl font-semibold leading-[1.55] tracking-tight">{children}</h1>,
        h2: ({ children }) => <h2 className="scroll-m-20 border-b pb-2 text-lg font-semibold leading-[1.55] tracking-tight first:mt-0">{children}</h2>,
        h3: ({ children }) => <h3 className="scroll-m-20 text-base font-semibold leading-[1.5] tracking-tight">{children}</h3>,
        ul: ({ children }) => (
          <ul
            className={cn(
              'my-3 ml-6 list-disc marker:text-muted-foreground/75 text-foreground/86',
              densityMode === 'compact' ? 'space-y-1 text-[12px] leading-5.25' : 'space-y-1.25 text-[13px] leading-5.75'
            )}
          >
            {children}
          </ul>
        ),
        ol: ({ children }) => (
          <ol
            className={cn(
              'my-3 ml-6 list-decimal marker:text-muted-foreground/75 text-foreground/86',
              densityMode === 'compact' ? 'space-y-1 text-[12px] leading-5.25' : 'space-y-1.25 text-[13px] leading-5.75'
            )}
          >
            {children}
          </ol>
        ),
        li: ({ children }) => <li className={densityMode === 'compact' ? 'leading-5.25' : 'leading-5.75'}>{children}</li>,
        blockquote: ({ children }) => (
          <blockquote className={cn('mt-4 border-l-2 border-border pl-4 italic text-muted-foreground', densityMode === 'compact' ? 'text-[12px] leading-5.25' : 'text-[13px] leading-5.75')}>
            {children}
          </blockquote>
        ),
        table: ({ children }) => (
          <div className={cn('my-3 overflow-hidden shadow-none ring-0', SURFACE_CARD_TOKENS.radius, SURFACE_CARD_TOKENS.border, SURFACE_CARD_TOKENS.background)}>
            <table className={cn('w-full border-collapse text-foreground/82', densityMode === 'compact' ? 'text-[11.5px]' : 'text-[12px]')}>
              {children}
            </table>
          </div>
        ),
        thead: ({ children }) => <thead className={SURFACE_CARD_TOKENS.headerBackground}>{children}</thead>,
        tbody: ({ children }) => <tbody className="[&_tr:last-child_td]:border-b-0">{children}</tbody>,
        th: ({ children }) => (
          <th className={cn(
            'border-b border-border/70 px-4 text-left font-medium tracking-tight text-foreground/84',
            densityMode === 'compact' ? 'py-1 text-[11px]' : 'py-1.75 text-[12px]'
          )}>
            {children}
          </th>
        ),
        td: ({ children }) => (
          <td className={cn(
            'border-b border-border/60 px-4 align-top font-normal text-foreground/80',
            densityMode === 'compact' ? 'py-1 text-[11.5px] leading-4.5' : 'py-1.75 text-[12px] leading-5'
          )}>
            {children}
          </td>
        ),
        hr: () => null,
        a: ({ children, href }) => (
          <a
            href={href}
            target="_blank"
            rel="noreferrer"
            className="text-primary underline decoration-primary/40 underline-offset-4 hover:decoration-primary"
          >
            {children}
          </a>
        ),
        code: ({ className, children, ...props }) => {
          const isBlock = className?.includes('language-')
          if (isBlock) {
            return (
              <code className={cn('block overflow-x-auto rounded-none bg-transparent p-0 font-mono text-[12px] leading-5 text-foreground', className)} {...props}>
                {children}
              </code>
            )
          }

          return (
            <code
              className="rounded-[3px] bg-muted px-1.5 py-0.5 font-mono text-[12px] font-normal text-foreground/78"
              {...props}
            >
              {children}
            </code>
          )
        },
        pre: ({ children }) => <CodeBlock densityMode={densityMode}>{children}</CodeBlock>,
      }}
    >
      {normalizedContent}
    </ReactMarkdown>
  )
}, (prev, next) =>
  prev.contentHash === next.contentHash &&
  prev.content === next.content &&
  prev.densityMode === next.densityMode
)
