/**
 * InfoPanel — right-side orange information panel.
 *
 * Displays step-specific guidance, status cards, and summaries
 * on the orange gradient background (30% right panel).
 *
 * Uses white/warm text for WCAG AA compliance against dark orange.
 */

import type { ReactNode } from 'react';
import { cn } from '@/lib/utils';

interface InfoPanelProps {
  /** Step label, e.g. "STEP 1: WELCOME" */
  stepLabel: string;
  /** Panel title / headline */
  title: string;
  /** Guidance bullet points */
  bullets?: string[];
  /** Optional custom content (cards, lists, etc.) */
  children?: ReactNode;
  className?: string;
}

export function InfoPanel({
  stepLabel,
  title,
  bullets,
  children,
  className,
}: InfoPanelProps) {
  return (
    <div
      className={cn(
        'flex flex-col gap-4 px-6 py-6 text-white',
        className,
      )}
    >
      {/* Step label */}
      <span
        className="text-token-xs font-semibold tracking-widest uppercase"
        style={{ color: 'rgba(255,255,255,0.6)' }}
      >
        {stepLabel}
      </span>

      {/* Title */}
      <h2 className="text-token-xl font-semibold leading-tight">
        {title}
      </h2>

      {/* Bullet points */}
      {bullets && bullets.length > 0 && (
        <ul className="flex flex-col gap-2">
          {bullets.map((bullet, i) => (
            <li
              key={i}
              className="text-token-sm leading-relaxed"
              style={{ color: 'rgba(255,255,255,0.85)' }}
            >
              {bullet}
            </li>
          ))}
        </ul>
      )}

      {/* Custom content (cards, lists, etc.) */}
      {children}
    </div>
  );
}

/** Card component for use inside InfoPanel (white container on orange bg). */
export function InfoCard({
  children,
  className,
}: {
  children: ReactNode;
  className?: string;
}) {
  return (
    <div
      className={cn(
        'rounded-lg px-4 py-3 text-token-sm',
        className,
      )}
      style={{
        background: 'rgba(255, 255, 255, 0.12)',
        backdropFilter: 'blur(8px)',
        border: '1px solid rgba(255, 255, 255, 0.15)',
      }}
    >
      {children}
    </div>
  );
}
