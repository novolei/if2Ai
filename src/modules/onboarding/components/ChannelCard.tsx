/**
 * ChannelCard — selectable channel card for the ChannelSetup step.
 *
 * Displays a channel with icon, name, and connection status.
 * Used in grid layouts for both common and additional channels.
 */

import { Check, Loader2 } from 'lucide-react';
import { cn } from '@/lib/utils';
import type { Channel, TestResult } from '../types';

interface ChannelCardProps {
  channel: Channel;
  isSelected: boolean;
  isConnected: boolean;
  isTesting: boolean;
  testResult: TestResult | null;
  onClick: () => void;
  className?: string;
}

export function ChannelCard({
  channel,
  isSelected,
  isConnected,
  isTesting,
  testResult,
  onClick,
  className,
}: ChannelCardProps) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        'flex items-center gap-2.5 rounded-lg border px-3.5 py-3 text-left transition-all',
        'hover:shadow-token-sm hover:border-brand-orange/50',
        'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand-orange',
        isSelected && 'border-brand-orange bg-brand-orange/5 shadow-token-sm',
        isConnected && 'border-status-success/50 bg-status-success-bg/20',
        isTesting && 'border-border bg-muted/30',
        testResult && !testResult.success && 'border-status-error/50 bg-status-error-bg/20',
        className,
      )}
    >
      {/* Icon */}
      <div
        className={cn(
          'flex h-7 w-7 items-center justify-center rounded-md border bg-white',
          isConnected ? 'border-status-success/30' : 'border-border',
        )}
      >
        <span className="text-token-sm">{channel.icon}</span>
      </div>

      {/* Name */}
      <span className="text-token-sm font-medium text-foreground flex-1 min-w-0 truncate">
        {channel.name}
      </span>

      {/* Status indicator */}
      {isTesting && (
        <Loader2 className="h-4 w-4 animate-spin text-brand-orange shrink-0" />
      )}
      {isConnected && (
        <div className="flex h-5 w-5 items-center justify-center rounded-full bg-status-success text-white shrink-0">
          <Check className="h-3 w-3" />
        </div>
      )}
      {testResult && !testResult.success && !isTesting && (
        <span className="text-token-xs text-status-error shrink-0">
          !
        </span>
      )}
    </button>
  );
}
