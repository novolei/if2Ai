/**
 * StepHeader — top bar with step progress indicator and title.
 *
 * Shows the current step number (e.g. "1/6"), a row of 6 dot indicators,
 * and the step title.
 */

import { cn } from '@/lib/utils';

const STEP_LABELS = [
  'Welcome',
  'System Check',
  'Security',
  'Provider',
  'Channel',
  'Activation',
];

interface StepHeaderProps {
  currentStep: number;
  title: string;
  className?: string;
}

export function StepHeader({ currentStep, title, className }: StepHeaderProps) {
  const stepIndex = Math.max(1, Math.min(6, currentStep));

  return (
    <div className={cn('flex flex-col gap-3 px-8 pb-4', className)}>
      {/* Step number + progress dots */}
      <div className="flex items-center gap-3">
        <span className="text-token-sm font-medium text-muted-foreground">
          {stepIndex}/6
        </span>
        <div className="flex items-center gap-1.5">
          {STEP_LABELS.map((_, i) => {
            const n = i + 1;
            const isActive = n === stepIndex;
            const isCompleted = n < stepIndex;
            return (
              <div
                key={n}
                className={cn(
                  'w-2 h-2 rounded-full transition-all duration-200',
                  isActive && 'bg-brand-orange scale-110',
                  isCompleted && 'bg-status-success',
                  !isActive && !isCompleted && 'bg-border',
                )}
                title={STEP_LABELS[i]}
              />
            );
          })}
        </div>
      </div>

      {/* Step title */}
      <h1 className="text-token-2xl font-semibold text-foreground font-sans">
        {title}
      </h1>
    </div>
  );
}
