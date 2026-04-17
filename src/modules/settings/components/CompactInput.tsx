import { forwardRef } from 'react'
import { cn } from '@/lib/utils'

interface CompactInputProps extends React.InputHTMLAttributes<HTMLInputElement> {
  label?: string
}

export const CompactInput = forwardRef<HTMLInputElement, CompactInputProps>(
  ({ className, label, ...props }, ref) => {
    return (
      <div className="flex flex-col gap-1.5">
        {label && (
          <label className="text-[10.5px] font-semibold uppercase tracking-widest text-black/30">
            {label}
          </label>
        )}
        <input
          ref={ref}
          className={cn(
            'h-8 rounded-xl border border-black/[0.09] bg-black/[0.02] px-3 text-[12px] font-medium',
            'outline-none transition-all',
            'placeholder:text-muted-foreground/40 placeholder:font-normal',
            'focus:border-jade/40 focus:ring-[3px] focus:ring-jade/15',
            'disabled:opacity-40 disabled:cursor-not-allowed',
            className,
          )}
          {...props}
        />
      </div>
    )
  },
)

CompactInput.displayName = 'CompactInput'
