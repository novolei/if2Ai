/**
 * RecoveryCard — green-banner card shown when an interrupted run can
 * be resumed from a saved cursor.
 *
 * Extracted from `src/components/ui/chat-ui.tsx` (GF-01 PR-01) without
 * behaviour change. The collocated `summarizeDegradedReason` helper
 * preserves the legacy keyword-to-Chinese mapping.
 */

import * as React from 'react'
import { Check, RotateCcw } from 'lucide-react'
import { truncateText } from '../utils/text'

/**
 * Render the recovery banner. When a `resumeCursor` and `onResume`
 * handler are both supplied, the action button becomes interactive
 * (and shows the `isRecovering` spinner copy when busy).
 */
export function RecoveryCard({
  error,
  degradedReason,
  resumeCursor,
  isRecovering,
  onResume,
}: {
  error: string
  degradedReason?: string
  resumeCursor?: string
  isRecovering?: boolean
  onResume?: (resumeCursor: string) => void
}): React.JSX.Element {
  const reasonLabel = summarizeDegradedReason(degradedReason)

  return (
    <div className="relative w-full overflow-hidden rounded-[6px] border border-emerald-500/35 bg-emerald-500/10 px-3 py-2.5">
      <div className="absolute inset-y-0 left-0 w-1.5 rounded-l-[13px] bg-emerald-400/90" />
      <div className="flex items-start gap-2.5 pl-2 pr-1">
        <div className="mt-0.25 flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-emerald-500/15">
          <Check className="h-3.5 w-3.5 text-emerald-500" />
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <div className="text-[12.5px] font-medium leading-4.5 text-emerald-500">任务已部分完成</div>
            <span className="rounded-full border border-emerald-500/35 bg-emerald-500/12 px-2 py-0.5 text-[10px] font-medium leading-4 text-emerald-500">
              {isRecovering ? '恢复中' : '可继续恢复'}
            </span>
          </div>
          <div className="mt-0.5 text-[11.5px] leading-4.5 text-foreground/72">
            {isRecovering
              ? '正在基于已保留的恢复点继续补全未完成部分，不会重复已确认的副作用操作。'
              : '已保留本轮已确认的执行结果。继续后只补全未完成部分，不会重复已确认的副作用操作。'}
          </div>
          {reasonLabel ? (
            <div className="mt-1 text-[11px] leading-4 text-emerald-500/85">
              中断原因：{reasonLabel}
            </div>
          ) : null}
          <div className="mt-1.25 break-words rounded-md bg-background/35 px-2.5 py-1.25 text-[11px] font-mono leading-4 text-emerald-500/85">
            {truncateText(error, 500)}
          </div>
          {resumeCursor && onResume ? (
            <button
              type="button"
              onClick={() => onResume(resumeCursor)}
              disabled={isRecovering}
              className="mt-1.75 inline-flex items-center gap-1.5 rounded-md bg-emerald-500/14 px-2.5 py-1 text-[11px] font-medium text-emerald-500 transition-colors hover:bg-emerald-500/22 disabled:cursor-default disabled:opacity-60"
            >
              <RotateCcw className="h-3 w-3" />
              {isRecovering ? '正在恢复未完成任务…' : '继续未完成任务'}
            </button>
          ) : null}
        </div>
      </div>
    </div>
  )
}

/**
 * Map a `degradedReason` string (semicolon-delimited keyword list) to
 * the user-visible Chinese phrase. Returns `null` for empty inputs.
 */
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
