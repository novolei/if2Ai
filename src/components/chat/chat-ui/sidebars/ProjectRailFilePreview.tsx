/**
 * GF-01 PR-07 — extracted from `src/components/ui/chat-ui.tsx`.
 *
 * In-rail file preview surface: header row with back button + actions,
 * then a kind-aware body that renders one of (image / pdf iframe /
 * markdown block / code-fenced markdown). Render-equivalent move —
 * iframe + data-URL + ReactMarkdown plumbing preserved verbatim so the
 * preview pipeline cannot regress.
 */

import * as React from "react"
import { ArrowUpRight, ChevronRight, Quote, ScanSearch } from "lucide-react"
import ReactMarkdown from "react-markdown"
import remarkGfm from "remark-gfm"
import rehypeHighlight from "rehype-highlight"
import type { FilePreviewPayload } from "@/lib/tauri"
import { inferPreviewLanguage } from "./utils"

/**
 * Render a single file's preview inside the project-files rail.
 * Behaviour:
 *   - text / code: code-fenced markdown rendered with rehype-highlight
 *   - markdown:    raw markdown rendered with remark-gfm + rehype-highlight
 *   - image:       inline `<img>` with object-contain
 *   - pdf:         data-URL `<iframe>` (no sandbox attrs upstream)
 * Drag-out from the "拖入上下文" button publishes a plain-text payload so
 * the composer drop-zone can pick it up.
 */
export function ProjectRailFilePreview({
  preview,
  onBack,
  onOpenExternally,
  onQuoteIntoChat,
  onInsertIntoChat,
}: {
  preview: FilePreviewPayload
  onBack: () => void
  onOpenExternally: (preview: FilePreviewPayload) => void
  onQuoteIntoChat: (preview: FilePreviewPayload) => void
  onInsertIntoChat: (preview: FilePreviewPayload) => void
}) {
  const codeFence = React.useMemo(() => {
    const raw = preview.content ?? ''
    const language = inferPreviewLanguage(preview.name, preview.kind)
    return `\`\`\`${language}\n${raw}\n\`\`\``
  }, [preview.content, preview.kind, preview.name])
  const dataUrl = React.useMemo(() => {
    if (!preview.data_base64 || !preview.mime_type) return null
    return `data:${preview.mime_type};base64,${preview.data_base64}`
  }, [preview.data_base64, preview.mime_type])

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="flex items-center justify-between border-b border-border/60 px-1 pb-2.5">
        <button
          type="button"
          onClick={onBack}
          className="inline-flex items-center gap-1 rounded-full bg-muted px-2.5 py-1 text-[11px] text-muted-foreground transition-colors hover:bg-muted/80"
        >
          <ChevronRight className="h-3 w-3 rotate-180" />
          <span>返回目录</span>
        </button>
        <div className="min-w-0 truncate pl-3 text-[11px] text-muted-foreground">{preview.name}</div>
      </div>
      <div className="mt-2.5 flex flex-wrap items-center gap-1.5">
        <button
          type="button"
          onClick={() => onOpenExternally(preview)}
          className="inline-flex items-center gap-1.5 rounded-[6px] border border-border bg-surface-raised px-2.5 py-1.5 text-[11px] text-muted-foreground transition-all duration-150 hover:-translate-y-0.5 hover:bg-surface"
        >
          <ArrowUpRight className="h-3.5 w-3.5" />
          <span>在外部打开</span>
        </button>
        <button
          type="button"
          onClick={() => onQuoteIntoChat(preview)}
          className="inline-flex items-center gap-1.5 rounded-[6px] border border-border bg-surface-raised px-2.5 py-1.5 text-[11px] text-muted-foreground transition-all duration-150 hover:-translate-y-0.5 hover:bg-surface"
        >
          <Quote className="h-3.5 w-3.5" />
          <span>在聊天中引用</span>
        </button>
        <button
          type="button"
          draggable
          onClick={() => onInsertIntoChat(preview)}
          onDragStart={(event) => {
            const payload = `请把 \`${preview.path}\` 作为当前上下文文件一起考虑。`
            event.dataTransfer.setData('text/plain', payload)
            event.dataTransfer.effectAllowed = 'copy'
          }}
          className="inline-flex items-center gap-1.5 rounded-[6px] border border-border bg-surface-raised px-2.5 py-1.5 text-[11px] text-muted-foreground transition-all duration-150 hover:-translate-y-0.5 hover:bg-surface"
        >
          <ScanSearch className="h-3.5 w-3.5" />
          <span>拖入上下文</span>
        </button>
      </div>
      <div className="mt-3 min-h-0 overflow-auto rounded-[6px] border border-border bg-surface/88">
        {preview.kind === 'image' && dataUrl ? (
          <div className="flex min-h-full items-start justify-center p-4">
            <img src={dataUrl} alt={preview.name} className="max-h-full max-w-full rounded-[6px] object-contain shadow-xs" />
          </div>
        ) : preview.kind === 'pdf' && dataUrl ? (
          <iframe title={preview.name} src={dataUrl} className="h-full min-h-[520px] w-full rounded-[6px]" />
        ) : preview.kind === 'markdown' ? (
          <div className="px-4 py-4 text-[12px] leading-5.5 text-foreground/70">
            <ReactMarkdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeHighlight]}>
              {preview.content ?? ''}
            </ReactMarkdown>
          </div>
        ) : (
          <div className="px-4 py-4 text-[11px] leading-5.5 text-foreground/68">
            <ReactMarkdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeHighlight]}>
              {codeFence}
            </ReactMarkdown>
          </div>
        )}
      </div>
    </div>
  )
}
