/**
 * ErrorCard — renders assistant errors with actionable messaging.
 *
 * Extracted from `src/components/ui/chat-ui.tsx` (GF-01 PR-01) without
 * behaviour change. `classifyError` and `classifyErrorWithOutcome`
 * preserve the legacy keyword → palette/title/suggestion mapping.
 *
 * Supports both raw error strings and structured error hints.
 */

import * as React from 'react'
import { AlertTriangle, RotateCcw } from 'lucide-react'
import { truncateText } from '../utils/text'

/**
 * Render the error banner. When `isConnection` is true and `onRetry`
 * is provided, an inline "重试" button appears; when both
 * `resumeCursor` and `onResume` are provided, an additional "继续
 * 未完成任务" button is rendered.
 */
export function ErrorCard({
  error,
  taskOutcome,
  resumeCursor,
  onResume,
  onRetry,
}: {
  error: string
  taskOutcome?: string
  resumeCursor?: string
  onResume?: (resumeCursor: string) => void
  onRetry?: () => void
}): React.JSX.Element {
  // Classify error for user-friendly messaging
  const { title, suggestion, isConnection, kindLabel } = classifyError(error, taskOutcome)

  return (
    <div className="relative w-full overflow-hidden rounded-[6px] border border-rose-500/40 bg-rose-500/10 px-3 py-2.5">
      <div className="absolute inset-y-0 left-0 w-1.5 rounded-l-[13px] bg-rose-400/90" />
      <div className="flex items-start gap-2.5 pl-2 pr-1">
        <div className="mt-0.25 flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-rose-500/14">
          <AlertTriangle className="h-3.5 w-3.5 text-rose-500" />
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <div className="text-[12.5px] font-medium leading-4.5 text-rose-500">{title}</div>
            <span className="rounded-full border border-rose-500/40 bg-rose-500/12 px-2 py-0.5 text-[10px] font-medium leading-4 text-rose-500">
              {kindLabel}
            </span>
          </div>
          {suggestion && (
            <div className="mt-0.5 text-[11.5px] leading-4.5 text-foreground/72">{suggestion}</div>
          )}
          <div className="mt-1.25 break-words rounded-md bg-background/35 px-2.5 py-1.25 text-[11px] font-mono leading-4 text-rose-500/90">
            {truncateText(error, 500)}
          </div>
          {isConnection && onRetry && (
            <button
              type="button"
              onClick={onRetry}
              className="mt-1.75 flex items-center gap-1.5 rounded-md bg-rose-500/14 px-2.5 py-1 text-[11px] font-medium text-rose-500 transition-colors hover:bg-rose-500/22"
            >
              <RotateCcw className="h-3 w-3" />
              重试
            </button>
          )}
          {resumeCursor && onResume && (
            <button
              type="button"
              onClick={() => onResume(resumeCursor)}
              className="mt-1.75 ml-2 inline-flex items-center gap-1.5 rounded-md bg-rose-500/14 px-2.5 py-1 text-[11px] font-medium text-rose-500 transition-colors hover:bg-rose-500/22"
            >
              继续未完成任务
            </button>
          )}
        </div>
      </div>
    </div>
  )
}

/**
 * Classify an error string into user-friendly messaging.
 *
 * Thin wrapper around {@link classifyErrorWithOutcome} kept for parity
 * with the legacy chat-ui call surface.
 */
function classifyError(
  error: string,
  taskOutcome?: string,
): {
  title: string
  suggestion: string
  isConnection: boolean
  kindLabel: string
} {
  return classifyErrorWithOutcome(error, taskOutcome)
}

/**
 * Underlying classifier. Inspects substrings of the (lower-cased) error
 * plus an optional `taskOutcome` hint to pick a palette/title/suggestion
 * tuple. Order of checks matches the legacy chat-ui declaration.
 */
function classifyErrorWithOutcome(
  error: string,
  taskOutcome?: string,
): {
  title: string
  suggestion: string
  isConnection: boolean
  kindLabel: string
} {
  const lower = error.toLowerCase()

  if (taskOutcome === 'partial_success' || lower.includes('task_outcome] partial_success')) {
    return {
      title: '任务部分完成',
      suggestion: '本轮已有部分工具执行成功，但流在尾段中断。可继续发送“继续完成”来补全结果。',
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

  if (lower.includes('429') || lower.includes('rate limit') || lower.includes('too many requests')) {
    return {
      title: '请求频率受限',
      suggestion: 'API 调用已达上限，请稍后再试。',
      isConnection: false,
      kindLabel: '限流',
    }
  }

  if (lower.includes('500') || lower.includes('502') || lower.includes('503') || lower.includes('504')) {
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
