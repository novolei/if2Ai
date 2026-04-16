/**
 * CheckItem — system check item with status indicator.
 *
 * Used in SystemCheckStep to display CPU, GPU, and Node.js detection results.
 * Supports four states: Pending, Running, Pass, and Fail.
 *
 * Accessibility: role="status" with aria-label, status icons for all states,
 * focus-visible ring, prefers-reduced-motion support.
 */

import { Check, X, Loader2, AlertCircle } from 'lucide-react';
import { cn } from '@/lib/utils';
import type { CheckStatus } from '../types';

interface CheckItemProps {
  /** Label for the check (e.g. "CPU", "GPU", "Node.js") */
  label: string;
  /** Detail text (e.g. architecture, version) */
  detail?: string;
  /** Current check status from backend */
  status: CheckStatus;
  className?: string;
}

/** Extract a human-readable status from CheckStatus. */
function statusLabel(status: CheckStatus): string {
  switch (status.status) {
    case 'Pass':
      return '通过';
    case 'Fail':
      return status.reason;
    case 'Running':
      return '检测中...';
    case 'Pending':
      return '等待检测';
  }
}

export function CheckItem({ label, detail, status, className }: CheckItemProps) {
  const isRunning = status.status === 'Running';
  const isPass = status.status === 'Pass';
  const isFail = status.status === 'Fail';
  const isPending = status.status === 'Pending';

  return (
    <div
      role="status"
      aria-label={`${label}: ${statusLabel(status)}`}
      tabIndex={0}
      className={cn(
        'flex items-start gap-3 rounded-lg px-4 py-3 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand-orange',
        isPass && 'bg-status-success-bg/30',
        isFail && 'bg-status-error-bg/30',
        isRunning && 'bg-muted/30',
        isPending && 'bg-transparent',
        className,
      )}
    >
      {/* Status icon */}
      <div
        className={cn(
          'shrink-0 mt-0.5 flex h-6 w-6 items-center justify-center rounded-full',
          isPass && 'bg-status-success text-white',
          isFail && 'bg-status-error text-white',
          isRunning && 'bg-muted text-brand-orange',
          isPending && 'bg-muted/50 text-muted-foreground/40',
        )}
        aria-hidden="true"
      >
        {isPass && <Check className="h-3.5 w-3.5" />}
        {isFail && <X className="h-3.5 w-3.5" />}
        {isRunning && <Loader2 className="h-3.5 w-3.5 animate-spin motion-reduce:animate-none" />}
        {isPending && <AlertCircle className="h-3.5 w-3.5" />}
      </div>

      {/* Label + detail */}
      <div className="flex-1 min-w-0">
        <p
          className={cn(
            'text-token-sm font-medium',
            isFail ? 'text-status-error' : 'text-foreground',
          )}
        >
          {label}
        </p>
        {detail && (
          <p className="text-token-xs text-muted-foreground mt-0.5">
            {detail}
          </p>
        )}
      </div>

      {/* Status text */}
      <span
        className={cn(
          'text-token-xs shrink-0',
          isPass && 'text-status-success',
          isFail && 'text-status-error',
          isRunning && 'text-brand-orange',
          isPending && 'text-muted-foreground/40',
        )}
      >
        {statusLabel(status)}
      </span>
    </div>
  );
}
