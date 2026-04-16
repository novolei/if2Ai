/**
 * StepProgress — step progress indicator component.
 *
 * Renders a horizontal progress bar showing completion of the current step
 * alongside the 6-dot indicator row. Used within step pages to show
 * sub-progress (e.g. system checks, model download).
 */

import { cn } from '@/lib/utils';

interface StepProgressProps {
  /** Current step number (1-6) */
  currentStep: number;
  /** Sub-step progress within the current step (0-100) */
  progress?: number;
  /** Whether progress is actively loading */
  isLoading?: boolean;
  className?: string;
}

export function StepProgress({
  currentStep,
  progress,
  isLoading = false,
  className,
}: StepProgressProps) {
  const clampedStep = Math.max(1, Math.min(6, currentStep));
  const clampedProgress = Math.max(0, Math.min(100, progress ?? 0));
  const overallProgress = ((clampedStep - 1) * 100 + clampedProgress) / 6;

  return (
    <div className={cn('flex flex-col gap-2', className)}>
      {/* Overall progress bar */}
      <div className="flex items-center gap-3">
        <div className="flex-1 h-1.5 rounded-full bg-muted overflow-hidden">
          <div
            className={cn(
              'h-full rounded-full transition-all duration-500',
              isLoading && 'bg-brand-orange animate-pulse',
              !isLoading && 'bg-status-success',
            )}
            style={{ width: `${isLoading ? clampedProgress : overallProgress}%` }}
          />
        </div>
        <span className="text-token-xs text-muted-foreground tabular-nums min-w-[3ch]">
          {isLoading ? `${Math.round(clampedProgress)}%` : `${clampedStep}/6`}
        </span>
      </div>
    </div>
  );
}
