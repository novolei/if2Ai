/**
 * GF-01 PR-05 — extracted from `src/components/ui/chat-ui.tsx`.
 *
 * Renders the chat transcript surface: empty-state mount, virtual-list
 * vs direct-map switch (above {@link VIRTUAL_LIST_THRESHOLD} messages),
 * and primary-thinking-message ID derivation (only the first thinking
 * block per user→assistant turn renders the full block).
 *
 * Render-equivalent move from the original inline definition — props,
 * memo comparator and conditional branches are preserved verbatim.
 */

import * as React from "react"
import { VirtualMessageList } from "@/components/chat/VirtualMessageList"
import type { Message } from "@/components/ui/chat-ui"
import { EmptyState } from "@/components/chat/chat-ui/cards/EmptyState"
import { ChatMessage } from "@/components/chat/chat-ui/transcript/ChatMessage"

/** Mirrors the `DensityMode` union in `chat-ui.tsx` (kept local to avoid a cross-file type leak). */
type DensityMode = 'comfortable' | 'compact'
/** Mirrors the `FontMode` union in `chat-ui.tsx`. */
type FontMode = 'sans' | 'serif'

/**
 * Threshold above which the chat transcript switches from straight
 * `messages.map(...)` rendering to virtualised rendering. Sessions below
 * this size keep the original DOM shape (zero behaviour change); long
 * sessions automatically opt into virtualisation for stable scroll perf.
 *
 * Mirrors the constant defined in `chat-ui.tsx`; both must stay in sync.
 */
const VIRTUAL_LIST_THRESHOLD = 50

/**
 * ChatTranscript — virtual list / direct map switch + EmptyState mount +
 * `primaryThinkingMessageIds` derivation. Keep memo comparator literally
 * identical to the original chat-ui.tsx definition (PR-05 risk
 * mitigation).
 */
export const ChatTranscript = React.memo(function ChatTranscript({
  messages,
  bottomPadding,
  sessionTitle,
  projectLabel,
  defaultWorkdir,
  bottomRef,
  scrollRef,
  onScroll,
  onCopyMessage,
  onResumeFromCursor,
  copiedMessageId,
  densityMode,
  fontMode,
  isLeftPaneCollapsed,
  contentRightInset,
  contentMaxWidth,
}: {
  messages: Message[]
  bottomPadding: number
  sessionTitle: string
  projectLabel: string
  defaultWorkdir?: string
  bottomRef: React.RefObject<HTMLDivElement | null>
  scrollRef: React.RefObject<HTMLDivElement | null>
  onScroll: () => void
  onCopyMessage: (message: Message) => void
  onResumeFromCursor?: (resumeCursor: string) => void
  copiedMessageId: string | null
  densityMode: DensityMode
  fontMode: FontMode
  isLeftPaneCollapsed: boolean
  contentRightInset: number
  contentMaxWidth: number
}) {
  const primaryThinkingMessageIds = React.useMemo(() => {
    const primaryIds = new Set<string>()
    let seenThinkingThisTurn = false

    for (const item of messages) {
      if (item.role === 'user') {
        seenThinkingThisTurn = false
        continue
      }

      if (item.role === 'assistant' && item.thinking?.trim()) {
        if (!seenThinkingThisTurn) {
          primaryIds.add(item.id)
          seenThinkingThisTurn = true
        }
      }
    }

    return primaryIds
  }, [messages])

  return (
    <div
      ref={scrollRef}
      onScroll={onScroll}
      className="relative min-h-0 flex-1 overflow-x-hidden overflow-y-auto overscroll-x-none"
      style={{ overscrollBehaviorX: 'none', paddingRight: `${contentRightInset}px` }}
    >
      <div
        className="mx-auto flex w-full flex-col gap-4 px-10 pt-6 transition-[max-width] duration-300 ease-[cubic-bezier(0.22,1,0.36,1)]"
        style={{ paddingBottom: `${bottomPadding}px`, maxWidth: `${contentMaxWidth}px` }}
      >
        {messages.length === 0 ? (
          <EmptyState sessionTitle={sessionTitle} projectLabel={projectLabel} />
        ) : messages.length >= VIRTUAL_LIST_THRESHOLD ? (
          <VirtualMessageList
            messages={messages}
            scrollElementRef={scrollRef}
            estimatedItemSize={84}
            renderMessage={(msg) => (
              <div className={densityMode === 'compact' ? 'pb-2' : 'pb-3'}>
                <ChatMessage
                  message={msg}
                  onCopyMessage={onCopyMessage}
                  onResumeFromCursor={onResumeFromCursor}
                  isCopied={copiedMessageId === msg.id}
                  defaultWorkdir={defaultWorkdir}
                  isPrimaryThinkingMessage={primaryThinkingMessageIds.has(msg.id)}
                  densityMode={densityMode}
                  fontMode={fontMode}
                />
              </div>
            )}
          />
        ) : (
          <div className={densityMode === 'compact' ? 'space-y-2' : 'space-y-3'}>
            {messages.map((msg) => (
              <ChatMessage
                key={msg.id}
                message={msg}
                onCopyMessage={onCopyMessage}
                onResumeFromCursor={onResumeFromCursor}
                isCopied={copiedMessageId === msg.id}
                defaultWorkdir={defaultWorkdir}
                isPrimaryThinkingMessage={primaryThinkingMessageIds.has(msg.id)}
                densityMode={densityMode}
                fontMode={fontMode}
              />
            ))}
          </div>
        )}

        <div ref={bottomRef} />
      </div>
    </div>
  )
}, (prev, next) =>
  prev.messages === next.messages &&
  prev.bottomPadding === next.bottomPadding &&
  prev.sessionTitle === next.sessionTitle &&
  prev.projectLabel === next.projectLabel &&
  prev.copiedMessageId === next.copiedMessageId &&
  prev.densityMode === next.densityMode &&
  prev.fontMode === next.fontMode &&
  prev.isLeftPaneCollapsed === next.isLeftPaneCollapsed &&
  prev.contentRightInset === next.contentRightInset &&
  prev.contentMaxWidth === next.contentMaxWidth &&
  prev.onCopyMessage === next.onCopyMessage &&
  prev.onResumeFromCursor === next.onResumeFromCursor
)
