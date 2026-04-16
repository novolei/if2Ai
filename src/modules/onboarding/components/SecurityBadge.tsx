/**
 * SecurityBadge — security status indicator for the Activation step.
 *
 * Displays a compact badge showing the security posture of the onboarding
 * configuration. Used in the right panel of ActivationStep.
 */

import { Shield } from 'lucide-react';
import { cn } from '@/lib/utils';

interface SecurityBadgeProps {
  /** Whether all security checks pass */
  allSecure: boolean;
  className?: string;
}

export function SecurityBadge({ allSecure, className }: SecurityBadgeProps) {
  return (
    <div
      className={cn(
        'flex items-center gap-2 rounded-lg px-3 py-2',
        allSecure
          ? 'bg-status-success-bg/20'
          : 'bg-status-warning-bg/20',
        className,
      )}
    >
      <div
        className={cn(
          'flex h-6 w-6 items-center justify-center rounded-full',
          allSecure ? 'bg-status-success text-white' : 'bg-status-warning/30 text-status-warning',
        )}
      >
        <Shield className="h-3.5 w-3.5" />
      </div>
      <span
        className={cn(
          'text-token-xs font-medium',
          allSecure ? 'text-status-success' : 'text-status-warning',
        )}
      >
        {allSecure ? '安全已就绪' : '安全配置不完整'}
      </span>
    </div>
  );
}
