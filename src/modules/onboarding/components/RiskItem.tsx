/**
 * RiskItem — reusable risk disclosure item with checkbox.
 *
 * Used in SecurityConfirmStep to display 8 security risk disclosures.
 * Each item has a warning badge number, description, and optional checkbox.
 */

import { Check } from 'lucide-react';
import { cn } from '@/lib/utils';

interface RiskItemProps {
  /** Badge number (1-8) */
  number: number;
  /** Risk description text */
  description: string;
  /** Whether this is the final confirmation item (shows checkbox) */
  isConfirmation?: boolean;
  /** Whether the checkbox is checked (only for confirmation items) */
  checked?: boolean;
  /** Checkbox change handler (only for confirmation items) */
  onCheck?: (checked: boolean) => void;
  className?: string;
}

export function RiskItem({
  number,
  description,
  isConfirmation = false,
  checked = false,
  onCheck,
  className,
}: RiskItemProps) {
  return (
    <div
      className={cn(
        'flex items-start gap-3 rounded-lg p-3 transition-colors',
        isConfirmation
          ? checked
            ? 'bg-status-success-bg/60'
            : 'bg-background border border-border'
          : 'bg-status-warning-bg/40',
        className,
      )}
    >
      {/* Badge number */}
      <div
        className={cn(
          'flex-shrink-0 mt-0.5 flex h-6 w-6 items-center justify-center rounded-full text-token-xs font-bold',
          isConfirmation
            ? checked
              ? 'bg-status-success text-white'
              : 'bg-border text-muted-foreground'
            : 'bg-status-warning/20 text-status-warning',
        )}
      >
        {isConfirmation && checked ? (
          <Check className="h-3.5 w-3.5" />
        ) : (
          number
        )}
      </div>

      {/* Description */}
      <div className="flex-1 flex items-center gap-2">
        <p
          className={cn(
            'text-token-sm leading-relaxed',
            isConfirmation ? 'text-foreground font-medium' : 'text-foreground/80',
          )}
        >
          {description}
        </p>

        {/* Checkbox for confirmation item */}
        {isConfirmation && (
          <label className="flex-shrink-0 flex items-center gap-2 cursor-pointer">
            <input
              type="checkbox"
              checked={checked}
              onChange={(e) => onCheck?.(e.target.checked)}
              className="h-4 w-4 rounded border-border text-brand-orange focus:ring-brand-orange focus:ring-2"
            />
          </label>
        )}
      </div>
    </div>
  );
}
