/**
 * OnboardingLayout — 1.68:1 split layout for all Onboarding steps.
 *
 * Left panel (62.7%): white background, operation area.
 * Right panel (37.3%): orange gradient background (#CC4400 → #993300),
 *   state feedback and guidance area.
 *
 * Uses Paico design tokens — no hardcoded colors or spacing.
 */

import type { MouseEvent as ReactMouseEvent, ReactNode } from 'react';
import { cn } from '@/lib/utils';

interface OnboardingLayoutProps {
  children: ReactNode;
  rightPanel: ReactNode;
  className?: string;
  onWindowDrag?: (event: ReactMouseEvent<HTMLElement>) => void;
}

export function OnboardingLayout({
  children,
  rightPanel,
  className,
  onWindowDrag,
}: OnboardingLayoutProps) {
  return (
    <div
      className={cn(
        'flex h-screen w-full cursor-default overflow-hidden',
        className,
      )}
      onMouseDown={onWindowDrag}
    >
      {/* Left operation area — 62.7% (1.68:1 ratio), white background */}
      <div className="flex flex-[1.68] flex-col bg-background overflow-y-auto">
        {children}
      </div>

      {/* Right state panel — 37.3% (1.68:1 ratio), orange gradient with
          subtle diagonal stripe overlay, rounded-l-3xl (24px) on both
          left corners (two stops larger than default xl) */}
      <div
        className="flex-1 overflow-hidden rounded-l-3xl"
        style={{
          background: `
            repeating-linear-gradient(
              45deg,
              transparent,
              transparent 4px,
              rgba(255,255,255,0.06) 4px,
              rgba(255,255,255,0.06) 8px
            ),
            linear-gradient(180deg, var(--brand-orange-dark, #CC4400) 0%, var(--brand-orange-dark, #993300) 100%)
          `,
        }}
      >
        <div className="flex h-full flex-col overflow-y-auto">
          {rightPanel}
        </div>
      </div>
    </div>
  );
}
