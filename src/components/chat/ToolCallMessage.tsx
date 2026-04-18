/**
 * ToolCallMessage — Standalone tool-call message renderer.
 *
 * This file is the canonical home for the tool-call message renderer,
 * extracted from the God-Component `src/components/ui/chat-ui.tsx` as
 * part of FE-G.
 *
 * ## Migration Status
 * The full `ToolCallMessage` implementation currently lives inside
 * `chat-ui.tsx` where it depends on many shared helper utilities:
 * `buildToolCallDisplay`, `getToolCallGlyph`, `normalizeToolStatus`,
 * `ToolStatusGlyph`, etc.
 *
 * Until those helpers are extracted into `src/lib/toolDisplay.ts` (a
 * follow-up task), this file declares the public interface and exports
 * `ToolCallMessageProps` so higher-level consumers can type against the
 * canonical location.
 *
 * Once the migration is complete:
 *   1. Move the `ToolCallMessage` implementation here.
 *   2. Have `chat-ui.tsx` import it from this file.
 *   3. Remove the inline definition from `chat-ui.tsx`.
 *
 * @see src/components/ui/chat-ui.tsx (current implementation, line ~2362)
 * @see src/components/memory/MemoryWriteCard.tsx (specialized memory_store renderer)
 */

import type { Message } from '@/modules/chat/types'

/**
 * Props for the `ToolCallMessage` component.
 * Declared here so consumers can type against the canonical location.
 */
export interface ToolCallMessageProps {
  /**
   * The tool-call message to render.
   * Must have `role === 'tool'`.
   */
  message: Message
  /**
   * Default working directory shown in tool call paths.
   * Used to display relative paths instead of absolute ones.
   */
  defaultWorkdir?: string
}
