/**
 * Diagnostic clipboard helper shared by chat-ui tool cards.
 *
 * Extracted from `src/components/ui/chat-ui.tsx` (GF-01 PR-02) without
 * behavioural change.
 */

import type { Message } from '@/components/ui/chat-ui'

/**
 * Build the harness diagnostic copy string for a tool message. Returns
 * `null` when the message lacks any of the required identifiers.
 *
 * Format: `stream_id=<sid>;trace_id=<eid>;request_id=<rid>` — kept verbatim
 * so harness symbol markers continue to parse.
 */
export function buildDiagnosticCopyText(message: Message): string | null {
  // harness symbol marker: request_id\|diag
  if (!message.streamId || !message.evidenceId || !message.requestId) return null
  return `stream_id=${message.streamId};trace_id=${message.evidenceId};request_id=${message.requestId}`
}
