/**
 * StepProgressBar — shared progress bar for all onboarding steps.
 *
 * Renders "安装流程" label, "{currentStep}/6" indicator, and 6 equal-width
 * dot/segment indicators at the top of each step.
 * - Active/completed segments: jade
 * - Upcoming segments: muted gray
 */

import { cn } from '@/lib/utils';

interface StepProgressBarProps {
  currentStep: number;
  totalSteps?: number;
  className?: string;
}

export function StepProgressBar({
  currentStep,
  totalSteps = 6,
  className,
}: StepProgressBarProps) {
  return (
    <div className={cn('w-full px-8 pt-8 pb-10', className)}>
      {/* Label row: "安装流程" left, "1/6" right */}
      <div className="flex items-center justify-between mb-2">
        <span className="text-token-base font-medium text-muted-foreground">
          安装流程
        </span>
        <span className="text-token-base font-medium text-muted-foreground">
          {currentStep}/{totalSteps}
        </span>
      </div>

      {/* 6-step equal-width progress bar */}
      <div className="flex items-center gap-2">
        {Array.from({ length: totalSteps }, (_, i) => {
          const n = i + 1;
          const isActive = n === currentStep;
          const isCompleted = n < currentStep;
          return (
            <div
              key={n}
              className={cn(
                'flex-1 h-2 rounded-full transition-all duration-200',
                isActive && 'bg-jade',
                isCompleted && 'bg-jade',
                !isActive && !isCompleted && 'bg-muted',
              )}
            />
          );
        })}
      </div>
    </div>
  );
}
