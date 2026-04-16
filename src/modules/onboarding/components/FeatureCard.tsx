/**
 * FeatureCard — reusable feature highlight card.
 *
 * Used in WelcomeStep to display 4 key features in a 2×2 grid.
 * Each card has an icon, title, and description.
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
      className={cn(
        'flex gap-3 rounded-lg border border-border bg-card p-4 transition-all duration-200 hover:shadow-token-sm hover:border-brand-orange/30',
        className,
      )}
    >
      {/* Icon container — white rounded background */}
      <div className="flex-shrink-0 mt-0.5 flex h-8 w-8 items-center justify-center rounded-md bg-brand-orange/10 text-brand-orange">
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
