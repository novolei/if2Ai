/**
 * ToolCallCard — renders a single tool-call message (args / status /
 * details + WriteToolDiffCard wrapper for `file_write`).
 *
 * Extracted from `src/components/ui/chat-ui.tsx` (GF-01 PR-04) without
 * behaviour change. Original symbol was `ToolCallMessage`; the new export
 * uses the cards/ subtree convention `ToolCallCard` and re-exports the
 * old name as an alias to avoid breaking any future callsite that still
 * imports `ToolCallMessage`.
 */

import * as React from 'react'
import { ChevronDown, MoreHorizontal } from 'lucide-react'
import { cn } from '@/lib/utils'
import { Button } from '@/components/ui/button'
import { Collapsible, CollapsibleContent } from '@/components/ui/collapsible'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { WriteToolDiffCard } from '@/components/chat/WriteToolDiffCard'
import { MemoryStoreToolCard } from '@/components/chat/chat-ui/cards/MemoryStoreToolCard'
import {
  buildToolCallDisplay,
  getToolCallGlyph,
  isToolFailureStatus,
  normalizeToolStatus,
  renderInlineToolSummary,
  ToolStatusGlyph,
} from '@/components/chat/chat-ui/utils/toolCallDisplay'
import type { Message } from '@/components/ui/chat-ui'

const WEB_SEARCH_NO_KEY_PREFIX = '[web_search: 当前使用 DuckDuckGo 免费搜索'

/**
 * Best-effort clipboard copy helper used by the tool-call dropdown menu.
 * Trims the input and silently logs failures so that copy actions never
 * surface as user-facing errors.
 */
async function copyTextToClipboard(text: string) {
  const value = text.trim()
  if (!value) return

  try {
    await navigator.clipboard.writeText(value)
  } catch (err) {
    console.error('Failed to copy text:', err)
  }
}

/**
 * Render a single tool-call entry in the chat transcript.
 *
 * Behaviour preserved from the legacy `ToolCallMessage` declaration:
 *   - `memory_store` with `deny`/`prompt` decision short-circuits to
 *     `MemoryStoreToolCard`; `allow` falls through to the generic card.
 *   - `file_write` completion expands into a `WriteToolDiffCard` preview
 *     when args + previous_content (or fallback) are present.
 *   - DuckDuckGo no-key web_search banner is preserved verbatim.
 */
export function ToolCallCard({
  message,
  defaultWorkdir,
}: {
  message: Message
  defaultWorkdir?: string
}) {
  const [expanded, setExpanded] = React.useState(false)
  const [isHovered, setIsHovered] = React.useState(false)

  // Specialized memory_store renderer — only used when the policy engine
  // produces a `deny` or `prompt` decision so the user notices it.  The
  // common `allow` path falls through to the generic tool-call card so
  // routine memory writes blend into the tool history like any other tool.
  if (
    message.toolName === 'memory_store' &&
    (message.policyDecision === 'deny' || message.policyDecision === 'prompt')
  ) {
    return <MemoryStoreToolCard message={message} />
  }
  const status = normalizeToolStatus(message)
  const display = buildToolCallDisplay(message, defaultWorkdir, status)
  const isFailed = isToolFailureStatus(status)
  const title = isFailed ? `执行失败：${display.title}` : display.title
  const hasDetails = display.details.length > 0
  const ToolGlyph = getToolCallGlyph(message.toolName, message.toolArgs)

  // Detect web_search no-key notice injected by backend.
  const showNoKeyBanner =
    message.toolName === 'web_search' &&
    typeof message.content === 'string' &&
    message.content.includes(WEB_SEARCH_NO_KEY_PREFIX)

  // file_write 专用 diff 预览：toolArgs.path + toolArgs.content 都齐
  // 时，展开后用 WriteToolDiffCard 渲染（图里那张绿色行号 +/- 卡片）。
  // 真实的 `previous_content` 由 backend 在结构化 JSON 结果里下发；
  // 解析失败 / 旧内容超出 64KiB 上限被裁掉时，回落为"全新增"渲染。
  // Failure states skip the diff preview to avoid presenting a rejected write
  // request as already applied.
  // "已写入"来展示。
  const writeArgs = (message.toolArgs ?? {}) as Record<string, unknown>
  const writePath = typeof writeArgs.path === 'string' ? writeArgs.path : ''
  const writeContent = typeof writeArgs.content === 'string' ? writeArgs.content : ''
  const writePreviousContent = React.useMemo(() => {
    if (typeof message.content !== 'string' || !message.content.trim()) return ''
    try {
      const parsed = JSON.parse(message.content) as Record<string, unknown>
      if (parsed && parsed.kind === 'file_write' && typeof parsed.previous_content === 'string') {
        return parsed.previous_content
      }
    } catch {
      // 旧版 backend 仍可能返回纯文本 "Successfully wrote to file: ..."；
      // 此时 previous_content 不可得，按全新增渲染。
    }
    return ''
  }, [message.content])
  const showWriteDiff =
    typeof message.toolName === 'string' &&
    message.toolName.includes('file_write') &&
    status === 'completed' &&
    Boolean(writePath) &&
    typeof writeArgs.content === 'string'

  return (
    <Collapsible open={expanded} onOpenChange={setExpanded}>
      <div
        className={cn(
          'group relative my-0.5 pl-4',
          isFailed && 'text-rose-600'
        )}
        onPointerEnter={() => setIsHovered(true)}
        onPointerLeave={() => setIsHovered(false)}
      >
        <div className={cn('absolute bottom-0 left-[5px] top-0 w-px bg-border/40', isFailed && 'bg-rose-200/80')} />

        <div className="flex items-start gap-1.5">
          <div className={cn('mt-[7px] h-1.5 w-1.5 shrink-0 rounded-full bg-muted-foreground/30', isFailed && 'bg-rose-400')} />

          <button
            type="button"
            onClick={() => setExpanded((value) => !value)}
            className={cn(
              'group flex min-w-0 flex-1 items-center gap-2 rounded-none px-0 py-[2px] text-left transition-colors duration-150',
              isFailed ? 'hover:text-rose-600/90' : 'hover:text-foreground/70'
            )}
            aria-label={expanded ? '折叠工具调用' : '展开工具调用'}
          >
            <div className="flex size-4 shrink-0 items-center justify-center text-muted-foreground/35">
              <ToolGlyph className="h-3.5 w-3.5" />
            </div>

            <div className="min-w-0 flex-1">
              <div className={cn('flex items-center gap-1.5 text-[13px] leading-5 tracking-[-0.01em]', isFailed ? 'text-rose-600/86' : 'text-muted-foreground/50')}>
                <span className="truncate">{title}</span>
                <ToolStatusGlyph status={status} />
              </div>
            </div>

            <ChevronDown
              className={cn(
                'h-3.5 w-3.5 shrink-0 text-muted-foreground/30 transition-all duration-150',
                expanded ? 'rotate-180 text-muted-foreground/50' : isHovered ? 'text-muted-foreground/50' : 'text-muted-foreground/25'
              )}
            />
          </button>

          <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button
              type="button"
              variant="ghost"
              size="icon"
              className="size-[18px] shrink-0 rounded-full border-0 bg-transparent text-muted-foreground/15 opacity-0 transition-opacity duration-150 group-hover:opacity-100 hover:bg-transparent hover:text-muted-foreground/40"
              aria-label="工具调用菜单"
              onClick={(event) => event.stopPropagation()}
            >
                <MoreHorizontal className="size-[12px]" />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" sideOffset={6} className="w-[172px]">
              <DropdownMenuItem
                onSelect={() => {
                  void copyTextToClipboard(display.copyText || display.title)
                }}
              >
                复制摘要
              </DropdownMenuItem>
              {display.resultCopyText ? (
                <DropdownMenuItem
                  onSelect={() => {
                    void copyTextToClipboard(display.resultCopyText as string)
                  }}
                >
                  复制结果
                </DropdownMenuItem>
              ) : null}
              {display.diagnosticCopyText ? (
                <DropdownMenuItem
                  onSelect={() => {
                    void copyTextToClipboard(display.diagnosticCopyText as string)
                  }}
                >
                  复制诊断串
                </DropdownMenuItem>
              ) : null}
              <DropdownMenuItem
                onSelect={() => {
                  setExpanded((value) => !value)
                }}
              >
                {expanded ? '收起详情' : '展开详情'}
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>

        <CollapsibleContent className="overflow-hidden">
          <div className="ml-[10px] border-l-[1.5px] border-border/40 pl-3 pt-1">
            {showWriteDiff && (
              <div className="mb-2">
                <WriteToolDiffCard
                  path={writePath}
                  newContent={writeContent}
                  oldContent={writePreviousContent}
                />
              </div>
            )}
            {showNoKeyBanner && (
              <div className="mb-2 flex items-start gap-2 rounded-lg border border-amber-500/35 bg-amber-500/12 px-2.5 py-2 text-[11.5px] leading-5 text-amber-500">
                <span className="mt-0.5 shrink-0">⚠️</span>
                <span>
                  当前使用 DuckDuckGo 免费搜索，结果质量有限。
                  在{' '}
                  <span className="font-medium underline underline-offset-2 cursor-pointer">
                    设置 → Web Search
                  </span>{' '}
                  中添加 Tavily / Brave 等服务商以获得更准确的搜索结果。
                </span>
              </div>
            )}
            {hasDetails ? (
              <div className="flex flex-col gap-0.5">
                {display.details.map((line, index) => (
                  <div
                    key={`${line}-${index}`}
                    className={cn(
                      'flex min-w-0 items-center gap-1.5 text-[11.5px] leading-5 text-muted-foreground/78',
                      index === 0 && 'px-0.5 py-0.5 text-muted-foreground'
                    )}
                  >
                    {index === 0 ? (
                      <div className="flex size-4 shrink-0 items-center justify-center text-muted-foreground/28">
                        <ToolGlyph className="h-3.5 w-3.5" />
                      </div>
                    ) : (
                      <span className="mt-[1px] h-1 w-1 shrink-0 rounded-full bg-muted-foreground/20" />
                    )}
                    <span className="min-w-0 truncate">
                      {index === 0 ? renderInlineToolSummary(line) : <span className="font-mono">{line}</span>}
                    </span>
                  </div>
                ))}
              </div>
            ) : isFailed ? (
              <div className="text-[11.5px] leading-5 text-rose-500/72">执行失败</div>
            ) : null}
          </div>
        </CollapsibleContent>
      </div>
    </Collapsible>
  )
}

// Backwards-compat alias for the legacy `ToolCallMessage` symbol.
export { ToolCallCard as ToolCallMessage }
