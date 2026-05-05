/**
 * GF-01 PR-05 — extracted from `src/components/ui/chat-ui.tsx`.
 *
 * Renders a single chat message shell (user / assistant / tool) with all
 * the surrounding chips (memory / turn-cost / routing), thinking block
 * (primary vs summarised), copy button, voice button, slash-command card
 * dispatch, recovery / error card dispatch, and final-run-report card.
 *
 * Render-equivalent move from the original inline definition — props,
 * memo comparator, and conditional branches are preserved verbatim. The
 * only structural change is that `ThinkingSummaryNode` (which is private
 * to this component) lives in this same file.
 */

import * as React from "react"
import { cn } from "@/lib/utils"
import { MemoryChip } from "@/components/memory/MemoryChip"
import { TurnCostChip } from "@/components/chat/TurnCostChip"
import { RoutingChip } from "@/components/chat/RoutingChip"
import { SlashResultCard } from "@/components/chat/SlashResultCard"
import type { Message } from "@/components/ui/chat-ui"
import { ThinkingBlock } from "@/components/chat/chat-ui/cards/ThinkingBlock"
import { LoadingIndicator } from "@/components/chat/chat-ui/cards/LoadingIndicator"
import { FinalRunReportCard } from "@/components/chat/chat-ui/cards/FinalRunReportCard"
import { RecoveryCard } from "@/components/chat/chat-ui/cards/RecoveryCard"
import { ErrorCard } from "@/components/chat/chat-ui/cards/ErrorCard"
import { MessageCopyButton } from "@/components/chat/chat-ui/cards/MessageCopyButton"
import { ToolCallCard } from "@/components/chat/chat-ui/cards/ToolCallCard"
import { MarkdownContent } from "@/components/chat/chat-ui/transcript/MarkdownContent"
import { formatDuration, formatShortTime, summarizeThinkingText } from "@/components/chat/chat-ui/utils/text"
import { getAssistantStatusMeta } from "@/components/chat/chat-ui/utils/toolCallDisplay"
import { hashString } from "@/components/chat/chat-ui/utils/markdownNormalize"

/** Lazy-loaded message-level voice button (kept out of the entry bundle to
 *  avoid AudioContext init at app startup — Phase TTS-E). */
const MessageVoiceButtonLazy = React.lazy(() =>
  import('@/modules/chat/MessageVoiceButton').then((m) => ({ default: m.MessageVoiceButton }))
)

/** Mirrors the `DensityMode` union in `chat-ui.tsx` (kept local to avoid a cross-file type leak). */
type DensityMode = 'comfortable' | 'compact'
/** Mirrors the `FontMode` union in `chat-ui.tsx`. */
type FontMode = 'sans' | 'serif'

/**
 * ChatMessage — single message shell that dispatches user / assistant /
 * tool block rendering. Keep the memo comparator literally identical to
 * the original chat-ui.tsx definition (PR-05 risk mitigation).
 */
export const ChatMessage = React.memo(function ChatMessage({
  message,
  onCopyMessage,
  onResumeFromCursor,
  isCopied,
  defaultWorkdir,
  isPrimaryThinkingMessage,
  densityMode,
  fontMode,
}: {
  message: Message
  onCopyMessage: (message: Message) => void
  onResumeFromCursor?: (resumeCursor: string) => void
  isCopied: boolean
  defaultWorkdir?: string
  isPrimaryThinkingMessage?: boolean
  densityMode: DensityMode
  fontMode: FontMode
}) {
  const isUser = message.role === 'user'
  const isTool = message.role === 'tool'
  const hasThinking = Boolean(message.thinking?.trim())
  const hasContent = Boolean(message.content?.trim())
  const shortTime = formatShortTime(message.timestamp)
  const showCopyButton = isCopied
  const contentHash = React.useMemo(() => hashString(message.content), [message.content])
  const showStatusLabel = !isUser && !isTool && Boolean(message.statusLabel)
  const statusMeta = getAssistantStatusMeta(message)

  if (isTool) {
    return <ToolCallCard message={message} defaultWorkdir={defaultWorkdir} />
  }

  // Slash 命令的静态结果（来自 `executeSlashCommand`，由 App.tsx 在
  // 消息上打 `slashCommand` 标记）→ 用紧凑型单行卡片替代 markdown 气泡。
  // 只在 assistant 侧生效；isUser 早就走了 `if (isUser)` 分支。
  if (!isUser && !isTool && message.slashCommand && hasContent) {
    return (
      <div
        className={cn(
          'group flex flex-col items-start',
          densityMode === 'compact' ? 'gap-1.5' : 'gap-2.5',
        )}
      >
        <SlashResultCard
          slashCommand={message.slashCommand}
          content={message.content}
        />
        <div className="flex items-center gap-1.5 pl-1">
          <MessageCopyButton
            side="right"
            copied={isCopied}
            visible={showCopyButton}
            onClick={() => onCopyMessage(message)}
          />
          <div className="text-[11px] leading-none text-muted-foreground/70">{shortTime}</div>
        </div>
      </div>
    )
  }

  return (
    <div className={cn('group flex flex-col', fontMode === 'serif' ? 'font-serif' : 'font-sans', densityMode === 'compact' ? 'gap-1.5' : 'gap-2.5', isUser ? 'items-end' : 'items-start')}>
      {isUser ? (
        <div className={cn('flex flex-col items-end gap-1', densityMode === 'compact' ? 'max-w-[min(620px,78%)]' : 'max-w-[min(680px,78%)]')}>
          <div
            className={cn(
              'rounded-lg bg-secondary px-3 text-foreground/80 shadow-xs',
              densityMode === 'compact' ? 'py-1.5 text-[12px] leading-5.5' : 'py-2 text-[13px] leading-6'
            )}
          >
            <div className={cn('whitespace-pre-wrap break-words [overflow-wrap:anywhere]', fontMode === 'serif' ? 'font-serif' : 'font-sans')}>
              {message.content}
            </div>
          </div>
          <div className="flex items-center gap-1.5 pr-1">
            <MessageCopyButton
              side="left"
              copied={isCopied}
              visible={showCopyButton}
              onClick={() => onCopyMessage(message)}
            />
            <div className="text-[11px] leading-none text-muted-foreground/70">{shortTime}</div>
          </div>
        </div>
      ) : (
        <div className="w-full space-y-2.5">
          {message.isError && message.toolArgs?.rawError ? (
            (() => {
              const taskOutcome =
                typeof message.toolArgs.taskOutcome === 'string'
                  ? message.toolArgs.taskOutcome
                  : message.taskOutcome
              const resumeCursor =
                typeof message.toolArgs.resumeCursor === 'string'
                  ? message.toolArgs.resumeCursor
                  : message.resumeCursor
              const degradedReason =
                typeof message.toolArgs.degradedReason === 'string'
                  ? message.toolArgs.degradedReason
                  : message.degradedReason
              const rawError = String(message.toolArgs.rawError)

              return taskOutcome === 'partial_success' ? (
                <RecoveryCard
                  error={rawError}
                  degradedReason={degradedReason}
                  resumeCursor={resumeCursor}
                  isRecovering={Boolean(message.isRecovering)}
                  onResume={onResumeFromCursor}
                />
              ) : (
                <ErrorCard
                  error={rawError}
                  taskOutcome={taskOutcome}
                  resumeCursor={resumeCursor}
                  onResume={onResumeFromCursor}
                />
              )
            })()
          ) : (
            <>
              <div
                className={cn(
                  'pt-0 font-normal text-foreground/86',
                  densityMode === 'compact' ? 'text-[12px] leading-5.25' : 'text-[13px] leading-5.75'
                )}
              >
                {hasThinking && (
                  isPrimaryThinkingMessage ? (
                    <ThinkingBlock
                      thinking={message.thinking ?? ''}
                      thinkingTime={message.thinkingTime}
                    />
                  ) : (
                    <ThinkingSummaryNode
                      thinking={message.thinking ?? ''}
                      thinkingTime={message.thinkingTime}
                    />
                  )
                )}

                {!hasThinking && message.isStreaming ? (
                  <div className="mb-2.5 flex items-start">
                    <LoadingIndicator />
                  </div>
                ) : null}

                {showStatusLabel ? (
                  <div className={cn(
                    'mb-2 inline-flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-[11px] leading-4',
                    statusMeta.containerClass
                  )}>
                    <span className={cn('h-1.5 w-1.5 rounded-full', statusMeta.dotClass)} />
                    <span>{message.statusLabel}</span>
                  </div>
                ) : null}

                {hasContent ? (
                  <>
                    <MarkdownContent
                      content={message.content}
                      contentHash={contentHash}
                      densityMode={densityMode}
                    />
                    <div className="mt-1.5 flex items-center gap-1.5 pl-1">
                      <div className="text-[11px] leading-none text-muted-foreground/70">{shortTime}</div>
                      {/* Per-message TurnCost chip — placed before the copy
                          button so the user sees billable usage right after
                          the timestamp, mirroring Steward's `turn-cost-bar`
                          layout (see ChatArea.svelte). */}
                      {message.turnCost ? (
                        <TurnCostChip turnCost={message.turnCost} className="ml-1" />
                      ) : null}
                      <MessageCopyButton
                        side="right"
                        copied={isCopied}
                        visible={showCopyButton}
                        onClick={() => onCopyMessage(message)}
                      />
                      {/* Phase TTS-E：消息级语音播放按钮（有 agent voice 时才显示） */}
                      {!message.isStreaming ? (
                        <React.Suspense fallback={null}>
                          <MessageVoiceButtonLazy text={message.content} />
                        </React.Suspense>
                      ) : null}
                      {message.memoryContext && message.memoryContext.length > 0 ? (
                        <MemoryChip items={message.memoryContext} className="ml-1" />
                      ) : null}
                      {message.routing ? (
                        <RoutingChip routing={message.routing} className="ml-1" />
                      ) : null}
                    </div>
                  </>
                ) : null}
              </div>
            </>
          )}
          {message.finalRunReport ? (
            <FinalRunReportCard
              report={message.finalRunReport}
              onResume={onResumeFromCursor}
            />
          ) : null}
        </div>
      )}
    </div>
  )
}, (prev, next) =>
  prev.message === next.message &&
  prev.isCopied === next.isCopied &&
  prev.defaultWorkdir === next.defaultWorkdir &&
  prev.isPrimaryThinkingMessage === next.isPrimaryThinkingMessage &&
  prev.densityMode === next.densityMode &&
  prev.fontMode === next.fontMode &&
  prev.onCopyMessage === next.onCopyMessage &&
  prev.onResumeFromCursor === next.onResumeFromCursor
)

/**
 * ThinkingSummaryNode — compact 1-line summary used for non-primary
 * assistant turns within a single user→assistant cluster (only the first
 * thinking block of a turn renders the full {@link ThinkingBlock}).
 *
 * Private helper; not exported (mirrors the original file scoping).
 */
function ThinkingSummaryNode({
  thinking,
  thinkingTime,
}: {
  thinking: string
  thinkingTime?: number
}) {
  const durationLabel = thinkingTime ? formatDuration(thinkingTime) : ''
  const summary = summarizeThinkingText(thinking)
  const label = durationLabel ? `${summary} · ${durationLabel}` : summary

  return (
    <div className="relative mb-3 pl-4">
      <div className="absolute bottom-0 left-[5px] top-0 w-px bg-border/70" />
      <div className="flex items-center gap-1.5 rounded-none px-0 py-[2px] text-[12px] italic tracking-tight text-muted-foreground">
        <span className="h-1.5 w-1.5 rounded-full bg-muted-foreground/70" />
        <span className="truncate">{label}</span>
      </div>
    </div>
  )
}
