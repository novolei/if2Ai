/**
 * MemoryWriteCard — Specialized renderer for `memory_store` tool call results.
 *
 * Replaces the generic tool-call card when the tool is `memory_store`,
 * providing a purpose-built UI that surfaces:
 *   - The memory content being stored
 *   - The policy decision (allow / deny / prompt) from MemoryPolicyEngine
 *   - The reason code explaining the policy decision
 *   - The memory scope (global / project / session)
 *
 * This component is wired into the chat message renderer once
 * `MemoryAuditEmitter` is fully live on the backend.  Until then it can be
 * rendered standalone with mock props for development / storybook use.
 */

import { CheckCircle2, ShieldAlert, ShieldQuestion } from 'lucide-react'
import { cn } from '@/lib/utils'

// ── Types ─────────────────────────────────────────────────────────────────────

/** Policy decision produced by MemoryPolicyEngine (mirrors Rust PolicyDecision). */
export type PolicyDecision = 'allow' | 'deny' | 'prompt'

/** Memory scope mirrors Rust MemoryExecutionScope. */
export type MemoryScope = 'global' | 'project' | 'session'

export interface MemoryWriteCardProps {
  /** The memory content text being stored. */
  content: string
  /** Policy decision rendered by MemoryPolicyEngine. */
  policyDecision: PolicyDecision
  /**
   * Human-readable reason code explaining the policy decision.
   * Examples: "ALLOWED_BY_POLICY", "DENIED_HIGH_RISK_CONTENT",
   * "SHADOW_MODE_ALLOW", "USER_APPROVAL_REQUIRED"
   */
  reasonCode: string
  /** Memory scope this write targets. */
  scope?: MemoryScope
  /** Tool status at the time of rendering. */
  toolStatus?: 'queued' | 'running' | 'completed' | 'error'
  /** Whether the write is currently in-progress. */
  isStreaming?: boolean
  /** Additional CSS class names. */
  className?: string
}

// ── Helpers ───────────────────────────────────────────────────────────────────

const SCOPE_LABEL: Record<MemoryScope, string> = {
  global: '全局记忆',
  project: '项目记忆',
  session: '会话记忆',
}

const DECISION_CONFIG: Record<
  PolicyDecision,
  {
    label: string
    icon: typeof CheckCircle2
    containerClass: string
    iconClass: string
    badgeClass: string
  }
> = {
  // `allow` is rendered through the generic tool-call card by chat-ui.tsx;
  // this entry only exists so the type stays exhaustive.
  allow: {
    label: '已允许写入',
    icon: CheckCircle2,
    containerClass: 'border-status-success/30 bg-status-success-bg/30',
    iconClass: 'text-status-success',
    badgeClass: 'bg-status-success-bg text-status-success',
  },
  deny: {
    label: '已拒绝写入',
    icon: ShieldAlert,
    containerClass: 'border-status-error/30 bg-status-error-bg/30',
    iconClass: 'text-status-error',
    badgeClass: 'bg-status-error-bg text-status-error',
  },
  prompt: {
    label: '等待审批',
    icon: ShieldQuestion,
    containerClass: 'border-status-warning/30 bg-status-warning-bg/30',
    iconClass: 'text-status-warning',
    badgeClass: 'bg-status-warning-bg text-status-warning',
  },
}

/** Format a snake_case reason code into a human-readable label. */
function formatReasonCode(code: string): string {
  return code
    .toLowerCase()
    .replace(/_/g, ' ')
    .replace(/\b\w/g, (c) => c.toUpperCase())
}

// ── MemoryWriteCard ───────────────────────────────────────────────────────────

/**
 * Specialized card for `memory_store` tool call results.
 * Shows policy decision, reason code, scope, and content preview.
 */
export function MemoryWriteCard({
  content,
  policyDecision,
  reasonCode,
  scope = 'session',
  toolStatus,
  isStreaming = false,
  className,
}: MemoryWriteCardProps) {
  const config = DECISION_CONFIG[policyDecision]
  const DecisionIcon = config.icon

  const isPending = isStreaming || toolStatus === 'queued' || toolStatus === 'running'

  return (
    <div
      className={cn(
        'flex items-center gap-2 rounded-md border px-2.5 py-1.5 text-[11.5px] transition-colors duration-200',
        isPending ? 'border-border/50 bg-muted/30' : config.containerClass,
        className,
      )}
      aria-label={isPending ? '记忆写入 — 正在处理…' : `记忆写入 — ${config.label}`}
      title={
        isPending
          ? '正在写入记忆…'
          : `${config.label} · ${SCOPE_LABEL[scope]} · ${formatReasonCode(reasonCode)}`
      }
    >
      {isPending ? (
        <span
          className="inline-block h-3 w-3 animate-spin rounded-full border-2 border-muted-foreground/30 border-t-muted-foreground"
          aria-hidden
        />
      ) : (
        <DecisionIcon className={cn('h-3 w-3 shrink-0', config.iconClass)} aria-hidden />
      )}

      <span className="font-medium text-foreground/80">
        {isPending ? '正在写入记忆…' : config.label}
      </span>

      <span className="truncate text-foreground/55">{content}</span>

      {!isPending && (
        <span
          className={cn('ml-auto shrink-0 rounded px-1.5 py-px text-[10px] font-medium', config.badgeClass)}
        >
          {SCOPE_LABEL[scope]}
        </span>
      )}
    </div>
  )
}
