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
          <label className="text-[12px] text-muted-foreground">
            {label}
          </label>
        )}
        <input
          ref={ref}
          className={cn(
            'h-8 rounded-xl border border-border/60 bg-white/60 px-3 text-[12px] shadow-none',
            'focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-primary/40 focus:border-primary/40',
            'placeholder:text-muted-foreground/50',
            className,
          )}
          {...props}
        />
      </div>
    )
  },
)

CompactInput.displayName = 'CompactInput'
