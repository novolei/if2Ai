/**
 * DownloadProgress — embedded model download progress card.
 *
 * Displays the current download state with a progress bar,
 * file size info, and status indicator.
 *
 * Accessibility: aria-live="polite" for progress updates,
 * role="progressbar" with aria-valuenow, prefers-reduced-motion support.
 */

import { Check, Loader2, AlertTriangle } from 'lucide-react';
import { cn } from '@/lib/utils';

interface DownloadProgressProps {
  /** Download progress 0-100 */
  percent: number;
  /** Bytes downloaded so far */
  downloadedBytes?: number;
  /** Total bytes to download */
  totalBytes?: number;
  /** Whether download is actively running */
  isDownloading: boolean;
  /** Whether download has completed successfully */
  isComplete: boolean;
  /** Error message if download failed */
  error?: string;
  className?: string;
}

/** Format bytes to human-readable string (e.g. "128 MB"). */
function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const mb = bytes / (1024 * 1024);
  if (mb >= 1024) return `${(mb / 1024).toFixed(1)} GB`;
  return `${Math.round(mb)} MB`;
}

export function DownloadProgress({
  percent,
  downloadedBytes = 0,
  totalBytes = 0,
  isDownloading,
  isComplete,
  error,
  className,
}: DownloadProgressProps) {
  const clampedPercent = Math.max(0, Math.min(100, percent));
  const hasError = !!error;

  return (
    <div
      role="region"
      aria-live="polite"
      aria-label="Embedded model download progress"
      className={cn(
        'rounded-lg border px-4 py-3.5 transition-colors',
        hasError
          ? 'border-status-error/30 bg-status-error-bg/20'
          : isComplete
            ? 'border-status-success/30 bg-status-success-bg/20'
            : isDownloading
              ? 'border-brand-orange/30 bg-brand-orange/5'
              : 'border-border bg-muted/20',
        className,
      )}
    >
      {/* Header row: icon + label */}
      <div className="flex items-center gap-2 mb-2.5">
        <div
          className={cn(
            'flex h-6 w-6 items-center justify-center rounded-full',
            hasError
              ? 'bg-status-error text-white'
              : isComplete
                ? 'bg-status-success text-white'
                : isDownloading
                  ? 'bg-brand-orange/20 text-brand-orange'
                  : 'bg-muted text-muted-foreground/40',
          )}
          aria-hidden="true"
        >
          {hasError && <AlertTriangle className="h-3.5 w-3.5" />}
          {isComplete && <Check className="h-3.5 w-3.5" />}
          {isDownloading && <Loader2 className="h-3.5 w-3.5 animate-spin motion-reduce:animate-none" />}
          {!isDownloading && !isComplete && !hasError && (
            <span className="text-token-xs font-bold">i</span>
          )}
        </div>
        <span
          className={cn(
            'text-token-sm font-semibold',
            hasError
              ? 'text-status-error'
              : isComplete
                ? 'text-status-success'
                : 'text-foreground',
          )}
        >
          {hasError
            ? '下载失败'
            : isComplete
              ? '检测当前安装状态，已跳过下载'
              : isDownloading
                ? '正在下载 Embedded 模型...'
                : 'Embedded 模型下载'}
        </span>
      </div>

      {/* Progress bar */}
      <div className="h-2 rounded-full bg-muted overflow-hidden mb-2" aria-hidden="true">
        <div
          role="progressbar"
          aria-valuenow={Math.round(clampedPercent)}
          aria-valuemin={0}
          aria-valuemax={100}
          className={cn(
            'h-full rounded-full transition-all duration-300 motion-reduce:transition-none',
            hasError && 'bg-status-error',
            isComplete && 'bg-status-success',
            isDownloading && !hasError && 'bg-brand-orange',
            !isDownloading && !isComplete && !hasError && 'bg-muted-foreground/30',
          )}
          style={{ width: `${clampedPercent}%` }}
        />
      </div>

      {/* Info row */}
      <div className="flex items-center justify-between">
        {totalBytes > 0 ? (
          <span className="text-token-xs text-muted-foreground">
            {formatBytes(downloadedBytes)} / {formatBytes(totalBytes)}
          </span>
        ) : (
          <span className="text-token-xs text-muted-foreground">
            {isComplete ? '模型已就绪' : '等待检测...'}
          </span>
        )}
        <span
          className={cn(
            'text-token-xs tabular-nums',
            isComplete && 'text-status-success',
          )}
        >
          {Math.round(clampedPercent)}%
        </span>
      </div>

      {/* Error message */}
      {hasError && (
        <p className="text-token-xs text-status-error mt-2" role="alert">
          {error}
        </p>
      )}
    </div>
  );
}
