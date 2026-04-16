/**
 * ProviderCard — selectable provider card for the ProviderSetup step.
 *
 * Displays a provider with logo, name, status tag, and selection state.
 * Used in a grid layout showing all 14 built-in providers.
 */

import { Check, KeyRound, AlertCircle } from 'lucide-react';
import { cn } from '@/lib/utils';
import type { Provider, ProviderStatus } from '../../types';

interface ProviderCardProps {
  provider: Provider;
  isSelected: boolean;
  isConfigured: boolean;
  onClick: () => void;
  className?: string;
}

/** Render a status tag based on provider status. */
function StatusTag({ status }: { status: ProviderStatus }) {
  switch (status.status) {
    case 'Available':
      return (
        <span className="text-token-xs text-status-success font-medium">
          可用
        </span>
      );
    case 'ApiKeyRequired':
      return (
        <span className="text-token-xs text-brand-orange font-medium">
          需要 API Key
        </span>
      );
    case 'Unavailable':
      return (
        <span className="text-token-xs text-status-error font-medium">
          不可用
        </span>
      );
  }
}

/** Render a logo fallback (initials) when no image is available. */
function LogoFallback({ name }: { name: string }) {
  const initials = name
    .split(/[\s()（）]+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((w) => w[0])
    .join('')
    .toUpperCase()
    .slice(0, 2);

  return (
    <div className="flex h-full w-full items-center justify-center text-token-xs font-bold text-brand-orange">
      {initials}
    </div>
  );
}

export function ProviderCard({
  provider,
  isSelected,
  isConfigured,
  onClick,
  className,
}: ProviderCardProps) {
  const logoUrl = provider.logo_path
    ? `/assets/ProviderLogos/${provider.logo_path.split('/').pop()}`
    : null;

  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        'flex flex-col items-start gap-2.5 rounded-lg border px-3.5 py-3 text-left transition-all',
        'hover:shadow-token-sm hover:border-brand-orange/50',
        'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand-orange',
        isSelected && 'border-brand-orange bg-brand-orange/5 shadow-token-sm',
        isConfigured && 'border-status-success/50 bg-status-success-bg/20',
        provider.status.status === 'Unavailable' && 'opacity-50 cursor-not-allowed',
        className,
      )}
    >
      {/* Logo + selection indicator */}
      <div className="flex items-center gap-2 w-full">
        <div
          className={cn(
            'flex h-7 w-7 items-center justify-center rounded-md border bg-white',
            isSelected || isConfigured ? 'border-brand-orange/30' : 'border-border',
          )}
        >
          {logoUrl ? (
            <img
              src={logoUrl}
              alt={provider.name}
              className="h-5 w-5 object-contain"
              onError={(e) => {
                (e.target as HTMLImageElement).style.display = 'none';
                const parent = (e.target as HTMLImageElement).parentElement;
                if (parent) {
                  const fallback = document.createElement('div');
                  fallback.className = 'flex h-full w-full items-center justify-center text-token-xs font-bold text-brand-orange';
                  fallback.textContent = provider.name[0].toUpperCase();
                  parent.appendChild(fallback);
                }
              }}
            />
          ) : (
            <LogoFallback name={provider.name} />
          )}
        </div>

        {/* Selection checkmark */}
        {isConfigured && (
          <div className="ml-auto flex h-5 w-5 items-center justify-center rounded-full bg-status-success text-white">
            <Check className="h-3 w-3" />
          </div>
        )}
      </div>

      {/* Name */}
      <span className="text-token-sm font-medium text-foreground line-clamp-1">
        {provider.name}
      </span>

      {/* Status tag */}
      <StatusTag status={provider.status} />
    </button>
  );
}
