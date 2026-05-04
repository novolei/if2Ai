/**
 * MemoryStoreToolCard — wraps MemoryWriteCard with the chat-ui
 * tool-call shape (args / decision / scope / streaming flags).
 *
 * Extracted from `src/components/ui/chat-ui.tsx` (GF-01 PR-01) without
 * behaviour change. Argument-coercion order, defensive JSON parse, and
 * fallback decision/reason logic match the legacy declaration.
 */

import * as React from 'react'
import { MemoryWriteCard } from '@/components/memory/MemoryWriteCard'
import type { Message } from '@/components/ui/chat-ui'

/**
 * Render the canonical "memory_store" tool-call card. The user-supplied
 * `content` arg is preferred for the preview because the backend only
 * echoes a 120-char `content_preview` for prompt decisions and not for
 * allow / deny outcomes.
 */
export function MemoryStoreToolCard({ message }: { message: Message }): React.JSX.Element {
  const args = (message.toolArgs ?? {}) as Record<string, unknown>
  // The user-supplied content is the most accurate preview; the backend
  // only echoes a 120-char content_preview for prompt decisions, and not at
  // all for allow / deny.
  const argsContent =
    typeof args.content === 'string'
      ? (args.content as string)
      : typeof args.text === 'string'
        ? (args.text as string)
        : ''
  // Backend `pending_approval` payload includes a short preview when the
  // input args aren't reachable (rare, but parses defensively).
  let backendPreview = ''
  try {
    const parsed = JSON.parse(message.content || '') as Record<string, unknown>
    if (typeof parsed.content_preview === 'string') {
      backendPreview = parsed.content_preview
    }
  } catch {
    // Not a JSON payload (legacy tool flow); fall through.
  }
  const contentText = argsContent || backendPreview || message.content || ''

  // Prefer structured fields lifted from the tool result by App.tsx; fall
  // back to args (legacy) and finally to derived defaults.
  const scopeFromArgs =
    typeof args.scope === 'string' &&
    (args.scope === 'global' || args.scope === 'project' || args.scope === 'session')
      ? (args.scope as 'global' | 'project' | 'session')
      : undefined
  const scope: 'global' | 'project' | 'session' =
    message.memoryScope ?? scopeFromArgs ?? 'session'

  const decision = message.policyDecision ?? 'allow'
  const reasonCode =
    message.memoryReasonCode ??
    (typeof args.reason_code === 'string'
      ? (args.reason_code as string)
      : decision === 'deny'
        ? 'POLICY_DENIED'
        : decision === 'prompt'
          ? 'USER_APPROVAL_REQUIRED'
          : 'ALLOWED_BY_POLICY')

  return (
    <div className="my-1.5 pl-4">
      <MemoryWriteCard
        content={contentText}
        policyDecision={decision}
        reasonCode={reasonCode}
        scope={scope}
        toolStatus={message.toolStatus}
        isStreaming={message.toolStatus === 'queued' || message.toolStatus === 'running'}
      />
    </div>
  )
}
