/**
 * GF-01 PR-03 — extracted from `src/components/ui/chat-ui.tsx`.
 *
 * Renders a fenced markdown code block with a copy-to-clipboard button.
 * Falls back to plain narrative / keyword display modes when the block
 * content matches the heuristics in
 * `@/components/chat/chat-ui/utils/markdownNormalize`.
 *
 * Render-equivalent move — props/behaviour unchanged from the original
 * inline definition.
 */

import * as React from "react"
import { Check, Copy } from "lucide-react"
import { cn } from "@/lib/utils"
import {
  looksLikeKeywordLineBlock,
  looksLikeNarrativeTextBlock,
  normalizeCodeForDisplay,
  normalizeKeywordLineForDisplay,
  normalizeNarrativeTextForDisplay,
} from "@/components/chat/chat-ui/utils/markdownNormalize"

/** Mirrors the `DensityMode` union in `chat-ui.tsx`. */
type DensityMode = 'comfortable' | 'compact'

/** Same surface-card design tokens as `chat-ui.tsx`; kept in sync intentionally. */
const SURFACE_CARD_TOKENS = {
  radius: 'rounded-[6px]',
  border: 'border border-border/50',
  background: 'bg-surface',
  headerBackground: 'bg-surface-raised',
  headerDivider: 'border-b border-border/50',
  headerLabel: 'font-mono text-[10px] tracking-tight text-muted-foreground/45',
} as const

/** Render a fenced code block with a copy button (or a narrative/keyword fallback). */
export function CodeBlock({
  children,
  densityMode,
}: {
  children: React.ReactNode
  densityMode: DensityMode
}) {
  const [copied, setCopied] = React.useState(false)
  const [animateCopy, setAnimateCopy] = React.useState(false)
  const rawCode = extractCodeText(children)
  const displayCode = React.useMemo(() => normalizeCodeForDisplay(rawCode), [rawCode])

  if (looksLikeKeywordLineBlock(rawCode)) {
    return (
      <div
        className={cn(
          'my-2 text-foreground/82',
          densityMode === 'compact' ? 'text-[12px] leading-5.25' : 'text-[13px] leading-5.75'
        )}
      >
        <div className="whitespace-pre-wrap [overflow-wrap:anywhere]">{normalizeKeywordLineForDisplay(displayCode.trim())}</div>
      </div>
    )
  }

  if (looksLikeNarrativeTextBlock(rawCode)) {
    return (
      <div
        className={cn(
          'my-3 pl-4 text-muted-foreground italic',
          densityMode === 'compact' ? 'text-[12px] leading-5.25' : 'text-[13px] leading-5.75'
        )}
      >
        <div className="whitespace-pre-wrap [overflow-wrap:anywhere]">{normalizeNarrativeTextForDisplay(displayCode.trim())}</div>
      </div>
    )
  }

  const copyCode = async () => {
    const text = displayCode
    if (!text.trim()) return

    try {
      await navigator.clipboard.writeText(text)
      setCopied(true)
      setAnimateCopy(true)
      window.setTimeout(() => setCopied(false), 900)
      window.setTimeout(() => setAnimateCopy(false), 260)
    } catch (err) {
      console.error('Failed to copy code:', err)
    }
  }

  const content = (
    <div className={cn('my-3 overflow-hidden text-[12px] shadow-none ring-0', SURFACE_CARD_TOKENS.radius, SURFACE_CARD_TOKENS.border, SURFACE_CARD_TOKENS.background)}>
      <div className={cn(
        'flex items-center justify-between px-3.5',
        SURFACE_CARD_TOKENS.headerBackground,
        SURFACE_CARD_TOKENS.headerDivider,
        densityMode === 'compact' ? 'h-6' : 'h-7'
      )}>
        <div className={SURFACE_CARD_TOKENS.headerLabel}>
          code
        </div>
        <button
          type="button"
          className={cn(
            'inline-flex h-5 items-center gap-1 rounded px-1.5 text-[11px] text-muted-foreground transition-all duration-150 hover:bg-accent hover:text-accent-foreground',
            animateCopy && 'scale-[1.03] bg-emerald-500/10 text-emerald-600'
          )}
          onClick={copyCode}
          aria-label={copied ? '已复制' : '复制代码'}
        >
          {copied ? <Check className="size-3.5" /> : <Copy className="size-3.5" />}
          <span>{copied ? '已复制' : '复制'}</span>
        </button>
      </div>
      <div className={cn(SURFACE_CARD_TOKENS.background, densityMode === 'compact' ? 'px-3.5 pb-2.5 pt-2' : 'px-4 pb-3 pt-2.5')}>
        <pre className={cn(
          'm-0 overflow-x-auto whitespace-pre bg-transparent !bg-transparent font-mono text-foreground/86',
          densityMode === 'compact' ? 'text-[11.5px] leading-5.5' : 'text-[12px] leading-6'
        )}>
          {displayCode}
        </pre>
      </div>
    </div>
  )

  return content
}

/** Recursively extract plain text from a React node tree (used for the copy payload). */
function extractCodeText(node: React.ReactNode): string {
  if (Array.isArray(node)) return node.map(extractCodeText).join('')
  if (typeof node === 'string') return node
  if (typeof node === 'number' || typeof node === 'boolean' || node == null) return ''
  if (React.isValidElement(node)) {
    return extractCodeText((node.props as { children?: React.ReactNode }).children)
  }
  return ''
}
