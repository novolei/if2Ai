/**
 * StepNavigation — bottom navigation bar with Previous/Next buttons.
 *
 * Positioned at the bottom of the left operation panel.
 * Provides "Previous" and "Next" (CTA) buttons for step traversal.
 */

import { cn } from '@/lib/utils';

interface StepNavigationProps {
  currentStep: number;
  nextLabel?: string;
  prevLabel?: string;
  skipLabel?: string;
  onNext: () => void;
  onPrev: () => void;
  onSkip?: () => void;
  canGoNext?: boolean;
  canGoPrev?: boolean;
  nextLoading?: boolean;
  className?: string;
}

export function StepNavigation({
  currentStep,
  nextLabel = 'Next',
  prevLabel = 'Previous',
  skipLabel = '跳过',
  onNext,
  onPrev,
  onSkip,
  canGoNext = true,
  canGoPrev = currentStep > 1,
  nextLoading = false,
  className,
}: StepNavigationProps) {
  return (
    <div
      className={cn(
        'flex items-center justify-between px-8 py-5 border-t border-border',
        className,
      )}
    >
      {/* Previous button */}
      <button
        type="button"
        disabled={!canGoPrev}
        onClick={onPrev}
        className={cn(
          'px-4 py-2 rounded-md text-token-sm font-medium transition-all',
          canGoPrev
            ? 'text-muted-foreground hover:text-foreground hover:bg-surface-raised'
            : 'text-muted-foreground/40 cursor-not-allowed',
        )}
      >
        {prevLabel}
      </button>

      {/* Next / CTA button */}
      <div className="flex items-center gap-3">
        {onSkip && (
          <button
            type="button"
            onClick={onSkip}
            className="px-4 py-2 rounded-md text-token-xs text-muted-foreground hover:text-foreground transition-colors"
          >
            {skipLabel}
          </button>
        )}
        <button
          type="button"
          disabled={!canGoNext || nextLoading}
          onClick={onNext}
          className={cn(
            'px-6 py-2.5 rounded-md text-token-sm font-semibold text-white transition-all',
            canGoNext && !nextLoading
              ? 'bg-brand-orange hover:bg-brand-orange-dark active:translate-y-px shadow-token-sm'
              : 'bg-brand-orange/50 text-white/60 cursor-not-allowed',
            nextLoading && 'opacity-70',
          )}
        >
          {nextLoading ? 'Loading...' : nextLabel}
        </button>
      </div>
    </div>
  );
}
