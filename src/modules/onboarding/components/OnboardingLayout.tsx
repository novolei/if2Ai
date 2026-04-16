/**
 * OnboardingLayout — 70/30 split layout for all Onboarding steps.
 *
 * Left panel (70%): white background, operation area.
 * Right panel (30%): orange gradient background (#CC4400 → #993300),
 *   state feedback and guidance area.
 *
 * Uses Paico design tokens — no hardcoded colors or spacing.
 */

import type { ReactNode } from 'react';
import { cn } from '@/lib/utils';

interface OnboardingLayoutProps {
  children: ReactNode;
  rightPanel: ReactNode;
  className?: string;
}

export function OnboardingLayout({
  children,
  rightPanel,
  className,
}: OnboardingLayoutProps) {
  return (
    <div
      className={cn(
        'flex h-screen w-full overflow-hidden',
        className,
      )}
    >
      {/* Left operation area — 70%, white background */}
      <div className="w-[70%] flex flex-col bg-background overflow-y-auto">
        {children}
      </div>

      {/* Right state panel — 30%, orange gradient */}
      <div
        className="w-[30%] flex flex-col overflow-y-auto"
        style={{
          background:
            'linear-gradient(180deg, var(--brand-orange-dark, #CC4400) 0%, var(--brand-orange-dark, #993300) 100%)',
        }}
      >
        {rightPanel}
      </div>
    </div>
  );
}
