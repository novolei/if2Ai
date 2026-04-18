/**
 * ErrorCard — Assistant error display with actionable recovery options.
 *
 * Extracted from `src/components/ui/chat-ui.tsx` as part of the FE-G
 * God-Component split.  This is the canonical implementation; chat-ui.tsx
 * will import from here once the migration is complete.
 *
 * Two variants:
 *   - `ErrorCard` — full error display with retry / resume buttons
 *   - `RecoveryCard` — partial-success card with "continue task" CTA
 */

import { AlertTriangle, Check, RotateCcw } from 'lucide-react'

// ── Helpers ───────────────────────────────────────────────────────────────────

/** Truncate `text` to `maxLen` characters, appending '…' if cut. */
function truncateText(text: string, maxLen: number): string {
  if (text.length <= maxLen) return text
  return text.slice(0, maxLen) + '…'
}

/** Summarise a semi-colon-separated degraded reason into a short label. */
function summarizeDegradedReason(degradedReason?: string): string | null {
  if (!degradedReason) return null
  const normalized = degradedReason.split(';')[0]?.trim().toLowerCase()
  if (!normalized) return null
  if (normalized.includes('network_timeout')) return '模型流超时'
  if (normalized.includes('network_transport_error')) return '网络传输中断'
  if (normalized.includes('request_validation_error')) return '请求校验失败'
  if (normalized.includes('permission_error')) return '权限受限'
  if (normalized.includes('max_iterations_reached')) return '达到迭代上限'
  if (normalized.includes('read_only_success_before_failure')) return '只读工具已完成，但回答尾段中断'
  return degradedReason.split(';')[0] ?? null
}

interface ClassifiedError {
  title: string
  suggestion: string
  isConnection: boolean
  kindLabel: string
}

/** Classify an error string + task outcome into user-friendly messaging. */
function classifyError(error: string, taskOutcome?: string): ClassifiedError {
  const lower = error.toLowerCase()

  if (taskOutcome === 'partial_success' || lower.includes('task_outcome] partial_success')) {
    return {
      title: '任务部分完成',
      suggestion: '本轮已有部分工具执行成功，但流在尾段中断。可继续发送"继续完成"来补全结果。',
      isConnection: true,
      kindLabel: '部分成功',
    }
  }
  if (lower.includes('network_timeout:')) {
    return {
      title: '模型流超时',
      suggestion: '上游模型流在超时时间内未返回数据，建议重试或切换模型。',
      isConnection: true,
      kindLabel: '网络超时',
    }
  }
  if (lower.includes('permission_error:')) {
    return {
      title: '权限受限',
      suggestion: '当前权限模式不允许继续执行，请在权限弹窗中授权后重试。',
      isConnection: false,
      kindLabel: '权限错误',
    }
  }
  if (lower.includes('request_validation_error:')) {
    return {
      title: '请求格式不兼容',
      suggestion: '会话历史中的工具调用顺序与模型接口约束不一致，建议重试或新建会话。',
      isConnection: false,
      kindLabel: '请求校验失败',
    }
  }
  if (lower.includes('network_transport_error:')) {
    return {
      title: '网络传输中断',
      suggestion: '连接被中断或网络不稳定，请检查网络后重试。',
      isConnection: true,
      kindLabel: '网络中断',
    }
  }
  if (lower.includes('model_stream_error:')) {
    return {
      title: '模型流异常',
      suggestion: '模型流式输出异常终止，请稍后重试。',
      isConnection: false,
      kindLabel: '模型流断开',
    }
  }
  if (lower.includes('connection refused') || lower.includes('network') || lower.includes('dns')) {
    return {
      title: '网络连接失败',
      suggestion: '请检查网络连接和代理设置，确认 LLM 服务地址可访问。',
      isConnection: true,
      kindLabel: '网络错误',
    }
  }
  if (lower.includes('400') || lower.includes('invalidparameter') || lower.includes('invalid')) {
    return {
      title: '请求参数有误',
      suggestion: '工具定义或消息格式与服务端不兼容，请检查配置后重试。',
      isConnection: false,
      kindLabel: '请求错误',
    }
  }
  if (lower.includes('401') || lower.includes('unauthorized') || lower.includes('auth')) {
    return {
      title: '认证失败',
      suggestion: 'API Key 或认证令牌已过期，请在设置中更新。',
      isConnection: false,
      kindLabel: '认证错误',
    }
  }
  if (
    lower.includes('429') ||
    lower.includes('rate limit') ||
    lower.includes('too many requests')
  ) {
    return {
      title: '请求频率受限',
      suggestion: 'API 调用已达上限，请稍后再试。',
      isConnection: false,
      kindLabel: '限流',
    }
  }
  if (
    lower.includes('500') ||
    lower.includes('502') ||
    lower.includes('503') ||
    lower.includes('504')
  ) {
    return {
      title: '服务端异常',
      suggestion: 'LLM 服务端暂时不可用，请稍后重试。',
      isConnection: true,
      kindLabel: '服务异常',
    }
  }
  if (lower.includes('missing credential') || lower.includes('missing_credentials')) {
    return {
      title: '缺少 API 配置',
      suggestion: '请在 ~/.claude/settings.json 中配置 ANTHROPIC_AUTH_TOKEN 和 ANTHROPIC_BASE_URL。',
      isConnection: false,
      kindLabel: '配置缺失',
    }
  }
  return {
    title: 'Agent 执行异常',
    suggestion: '请检查后端日志或网络配置，确认 LLM 服务可用。',
    isConnection: false,
    kindLabel: '未知错误',
  }
}

// ── ErrorCard ─────────────────────────────────────────────────────────────────

export interface ErrorCardProps {
  /** Raw error string from the backend. */
  error: string
  /** Task outcome tag from `StreamTokenPayload`. */
  taskOutcome?: string
  /** Cursor token for resuming an interrupted task. */
  resumeCursor?: string
  /** Called when the user clicks "resume". */
  onResume?: (resumeCursor: string) => void
  /** Called when the user clicks "retry". */
  onRetry?: () => void
}

/**
 * Full error display for failed assistant turns.
 * Classifies the error string and shows actionable recovery options
 * (retry for connection errors, resume for partial-success).
 */
export function ErrorCard({
  error,
  taskOutcome,
  resumeCursor,
  onResume,
  onRetry,
}: ErrorCardProps) {
  const { title, suggestion, isConnection, kindLabel } = classifyError(error, taskOutcome)

  return (
    <div
      className="relative w-full overflow-hidden rounded-[6px] border border-rose-200/50 bg-rose-50/32 px-3 py-2.5"
      role="alert"
      aria-label={`错误：${title}`}
    >
      <div className="absolute inset-y-0 left-0 w-1.5 rounded-l-[13px] bg-rose-400/90" />
      <div className="flex items-start gap-2.5 pl-2 pr-1">
        <div className="mt-0.25 flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-rose-500/8">
          <AlertTriangle className="h-3.5 w-3.5 text-rose-500/92" aria-hidden />
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <div className="text-[12.5px] font-medium leading-4.5 text-rose-800/90">{title}</div>
            <span className="rounded-full border border-rose-300/70 bg-rose-100/70 px-2 py-0.5 text-[10px] font-medium leading-4 text-rose-700/90">
              {kindLabel}
            </span>
          </div>
          {suggestion && (
            <div className="mt-0.5 text-[11.5px] leading-4.5 text-rose-700/68">{suggestion}</div>
          )}
          <div className="mt-1.25 break-words rounded-md bg-white/32 px-2.5 py-1.25 text-[11px] font-mono leading-4 text-rose-600/72">
            {truncateText(error, 500)}
          </div>
          {isConnection && onRetry && (
            <button
              type="button"
              onClick={onRetry}
              className="mt-1.75 flex items-center gap-1.5 rounded-md bg-rose-100/68 px-2.5 py-1 text-[11px] font-medium text-rose-700 transition-colors hover:bg-rose-200/60"
            >
              <RotateCcw className="h-3 w-3" aria-hidden />
              重试
            </button>
          )}
          {resumeCursor && onResume && (
            <button
              type="button"
              onClick={() => onResume(resumeCursor)}
              className="mt-1.75 ml-2 inline-flex items-center gap-1.5 rounded-md bg-rose-100/68 px-2.5 py-1 text-[11px] font-medium text-rose-700 transition-colors hover:bg-rose-200/60"
            >
              继续未完成任务
            </button>
          )}
        </div>
      </div>
    </div>
  )
}

// ── RecoveryCard ──────────────────────────────────────────────────────────────

export interface RecoveryCardProps {
  /** Raw error / partial-success string from the backend. */
  error: string
  /** Human-readable reason for the interruption. */
  degradedReason?: string
  /** Cursor token for resuming the interrupted task. */
  resumeCursor?: string
  /** Whether the task is currently in the process of being resumed. */
  isRecovering?: boolean
  /** Called when the user clicks "continue". */
  onResume?: (resumeCursor: string) => void
}

/**
 * Partial-success card displayed when a task completed some steps before
 * being interrupted.  Offers a "continue unfinished task" CTA.
 */
export function RecoveryCard({
  error,
  degradedReason,
  resumeCursor,
  isRecovering,
  onResume,
}: RecoveryCardProps) {
  const reasonLabel = summarizeDegradedReason(degradedReason)

  return (
    <div
      className="relative w-full overflow-hidden rounded-[6px] border border-emerald-200/70 bg-emerald-50/55 px-3 py-2.5"
      role="status"
      aria-label="任务已部分完成"
    >
      <div className="absolute inset-y-0 left-0 w-1.5 rounded-l-[13px] bg-emerald-400/90" />
      <div className="flex items-start gap-2.5 pl-2 pr-1">
        <div className="mt-0.25 flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-emerald-500/10">
          <Check className="h-3.5 w-3.5 text-emerald-600" aria-hidden />
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <div className="text-[12.5px] font-medium leading-4.5 text-emerald-900/90">
              任务已部分完成
            </div>
            <span className="rounded-full border border-emerald-300/70 bg-emerald-100/80 px-2 py-0.5 text-[10px] font-medium leading-4 text-emerald-700/90">
              {isRecovering ? '恢复中' : '可继续恢复'}
            </span>
          </div>
          <div className="mt-0.5 text-[11.5px] leading-4.5 text-emerald-800/70">
            {isRecovering
              ? '正在基于已保留的恢复点继续补全未完成部分，不会重复已确认的副作用操作。'
              : '已保留本轮已确认的执行结果。继续后只补全未完成部分，不会重复已确认的副作用操作。'}
          </div>
          {reasonLabel && (
            <div className="mt-1 text-[11px] leading-4 text-emerald-700/75">
              中断原因：{reasonLabel}
            </div>
          )}
          <div className="mt-1.25 break-words rounded-md bg-white/45 px-2.5 py-1.25 text-[11px] font-mono leading-4 text-emerald-700/70">
            {truncateText(error, 500)}
          </div>
          {resumeCursor && onResume && (
            <button
              type="button"
              onClick={() => onResume(resumeCursor)}
              disabled={isRecovering}
              className="mt-1.75 inline-flex items-center gap-1.5 rounded-md bg-emerald-100/90 px-2.5 py-1 text-[11px] font-medium text-emerald-800 transition-colors hover:bg-emerald-200/80 disabled:cursor-default disabled:opacity-60"
            >
              <RotateCcw className="h-3 w-3" aria-hidden />
              {isRecovering ? '正在恢复未完成任务…' : '继续未完成任务'}
            </button>
          )}
        </div>
      </div>
    </div>
  )
}
