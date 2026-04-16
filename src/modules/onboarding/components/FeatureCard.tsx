/**
 * FeatureCard — reusable feature highlight card.
 *
 * Used in WelcomeStep to display 4 key features in a 2×2 grid.
 * Each card has an icon, title, and description.
 *
 * Accessibility: role="article" with aria-label, focus-visible ring,
 * prefers-reduced-motion support.
 */

import type { ReactNode } from 'react';
import { cn } from '@/lib/utils';

interface FeatureCardProps {
  icon: ReactNode;
  title: string;
  description: string;
  className?: string;
}

export function FeatureCard({ icon, title, description, className }: FeatureCardProps) {
  return (
    <div
      role="article"
      aria-label={title}
      tabIndex={0}
      className={cn(
        'flex gap-3 rounded-lg border border-border bg-card p-4 transition-all focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand-orange',
        'hover:shadow-token-sm hover:border-brand-orange/30 active:translate-y-0.5 disabled:opacity-50 disabled:pointer-events-none',
        'motion-reduce:transition-none',
        className,
      )}
    >
      {/* Icon container — white rounded background */}
      <div
        className="shrink-0 mt-0.5 flex h-8 w-8 items-center justify-center rounded-md bg-brand-orange/10 text-brand-orange"
        aria-hidden="true"
      >
        {icon}
      </div>

      {/* Text content */}
      <div className="flex flex-col gap-0.5">
        <h3 className="text-token-sm font-semibold text-foreground">{title}</h3>
        <p className="text-token-xs text-muted-foreground leading-relaxed">
          {description}
        </p>
      </div>
    </div>
  );
}
