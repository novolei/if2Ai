/**
 * ChatMessage — Standalone chat message component.
 *
 * This file is the canonical home for the chat message renderer, extracted
 * from the God-Component `src/components/ui/chat-ui.tsx` as part of FE-G.
 *
 * ## Migration Status
 * The full `ChatMessage` implementation currently lives inside
 * `chat-ui.tsx` where it shares helper utilities (MarkdownContent,
 * CodeBlock, SkillsSlashReport, etc.) with the rest of the ChatUI.
 *
 * Until those helpers are extracted into their own modules, this file
 * declares the public interface and exports `ChatMessageProps` so that
 * higher-level consumers can type against the canonical location.
 *
 * Once the migration is complete:
 *   1. Move the `ChatMessage` implementation here.
 *   2. Have `chat-ui.tsx` import it from this file.
 *   3. Remove the inline definition from `chat-ui.tsx`.
 *
 * @see src/components/ui/chat-ui.tsx (current implementation, line ~2767)
 * @see src/components/chat/ThinkingBlock.tsx (already extracted)
 * @see src/components/chat/ErrorCard.tsx (already extracted)
 */

import type { Message } from '@/modules/chat/types'
import type { Dispatch, SetStateAction } from 'react'

/** Density mode options for the chat transcript. */
export type DensityMode = 'comfortable' | 'compact'

/** Font family options for the chat transcript. */
export type FontMode = 'sans' | 'serif'

/**
 * Props for the `ChatMessage` component.
 * Declared here so consumers can type against the canonical location.
 */
export interface ChatMessageProps {
  /** The message to render. */
  message: Message
  /** Callback for the "copy message" action. */
  onCopyMessage: (message: Message) => void
  /** Called when the user clicks "resume" on a partial-success card. */
  onResumeFromCursor?: (resumeCursor: string) => void
  /** Whether the copy-to-clipboard animation is active for this message. */
  isCopied: boolean
  /** Default working directory for tool call display. */
  defaultWorkdir?: string
  /** True for the most recent assistant turn — renders the full ThinkingBlock. */
  isPrimaryThinkingMessage?: boolean
  /** Controls the text size and spacing of message content. */
  densityMode: DensityMode
  /** Controls the font family of message content. */
  fontMode: FontMode
}

/**
 * @internal Re-export type so that `Dispatch<SetStateAction<DensityMode>>`
 * is available for parent components without knowing the internal type.
 */
export type DensityModeSetState = Dispatch<SetStateAction<DensityMode>>

/**
 * @internal Re-export type so that `Dispatch<SetStateAction<FontMode>>`
 * is available for parent components without knowing the internal type.
 */
export type FontModeSetState = Dispatch<SetStateAction<FontMode>>
