/**
 * InfoPanel — right-side orange information panel.
 *
 * Displays step-specific guidance, status cards, and summaries
 * on the orange gradient background (30% right panel).
 *
 * Uses the shared RightPanelHeader component for consistent branding.
 */

import type { ReactNode } from 'react';
import { cn } from '@/lib/utils';
import { RightPanelHeader } from './RightPanelHeader';

interface InfoPanelProps {
  /** Step label, e.g. "STEP 1: WELCOME" */
  stepLabel: string;
  /** Panel title / headline */
  title: string;
  /** Guidance bullet points */
  bullets?: string[];
  /** Optional show "设置中" badge (defaults to true) */
  showBadge?: boolean;
  /** Optional custom content (cards, lists, etc.) */
  children?: ReactNode;
  className?: string;
}

export function InfoPanel({
  stepLabel,
  title,
  bullets,
  showBadge,
  children,
  className,
}: InfoPanelProps) {
  return (
    <div className={cn('flex flex-col flex-1', className)}>
      {/* Shared header */}
      <RightPanelHeader
        stepLabel={stepLabel}
        title={title}
        bullets={bullets}
        showBadge={showBadge}
      />

      {/* Divider */}
      <div
        className="mx-6 h-px shrink-0"
        style={{ background: 'rgba(255,255,255,0.1)' }}
      />

      {/* Custom content (cards, lists, etc.) */}
      {children && (
        <div className="flex-1 overflow-y-auto px-6 py-4">{children}</div>
      )}
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
