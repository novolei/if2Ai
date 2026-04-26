/**
 * RightPanelHeader — shared right panel header for all onboarding steps.
 *
 * Displays on the orange gradient background. Shows:
 * - Top row: step-specific label + "设置中" badge (optional)
 * - App branding: If2Ai logo + name
 * - Step-specific title
 * - Optional bullet points
 *
 * Used by InfoPanel and ProviderSetupStep's custom right panel.
 */

import type { ReactNode } from 'react';
import { cn } from '@/lib/utils';

/** App icon URL for the right panel header. */
const APP_ICON_URL = new URL('../../../assets/app-icon.png', import.meta.url).href;

interface RightPanelHeaderProps {
  /** Step-specific label, e.g. "STEP 1: WELCOME" */
  stepLabel: string;
  /** Step-specific title / headline */
  title: string;
  /** Optional bullet points */
  bullets?: string[];
  /** Optional show "设置中" badge (defaults to true) */
  showBadge?: boolean;
  /** Optional custom content below header */
  children?: ReactNode;
  className?: string;
}

export function RightPanelHeader({
  stepLabel,
  title,
  bullets,
  showBadge = true,
  children,
  className,
}: RightPanelHeaderProps) {
  return (
    <div
      className={cn(
        'flex flex-col gap-4 px-6 py-6 text-white shrink-0',
        className,
      )}
    >
      {/* Top row: step label + status badge */}
      <div className="flex items-center justify-between">
        <span
          className="font-sans text-[11px] font-semibold tracking-widest uppercase"
          style={{ color: 'rgba(255,255,255,0.6)' }}
        >
          {stepLabel}
        </span>
        {showBadge && (
          <span
            className="rounded-full bg-white/15 px-2.5 py-0.5 font-sans text-[11px] font-medium"
            style={{ color: 'rgba(255,255,255,0.9)' }}
          >
            设置中
          </span>
        )}
      </div>

      {/* App branding: logo + If2Ai */}
      <div className="flex items-center gap-3">
        <div className="flex h-12 w-12 shrink-0 items-center justify-center overflow-hidden rounded-xl shadow-[0_1px_0_0.5px_rgba(255,255,255,0.46),0_0_0_0.5px_rgba(0,0,0,0.10),0_8px_20px_rgba(18,24,38,0.16)]">
          <img
            src={APP_ICON_URL}
            alt="If2Ai"
            className="h-full w-full object-cover"
            draggable={false}
          />
        </div>
        <span
          className="font-sans text-[20px] font-bold tracking-tight"
          style={{ color: 'rgba(255,255,255,0.95)' }}
        >
          If2Ai
        </span>
      </div>

      {/* Title */}
      <h2 className="font-sans text-[18px] font-semibold leading-tight text-white">
        {title}
      </h2>

      {/* Bullet points */}
      {bullets && bullets.length > 0 && (
        <ul className="flex flex-col gap-1.5">
          {bullets.map((bullet, i) => (
            <li
              key={i}
              className="font-sans text-[12px] leading-relaxed flex items-start gap-2"
              style={{ color: 'rgba(255,255,255,0.85)' }}
            >
              <span className="mt-1.5 h-1 w-1 shrink-0 rounded-full bg-white/60" />
              {bullet}
            </li>
          ))}
        </ul>
      )}

      {/* Optional custom content */}
      {children}
    </div>
  );
}
