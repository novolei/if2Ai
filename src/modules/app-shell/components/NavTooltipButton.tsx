import type { ComponentType } from 'react'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import { cn } from '@/lib/utils'

export function NavTooltipButton({
  icon: Icon,
  label,
  active = false,
  ghost = false,
  variant = 'default',
  iconClassName,
  disabled = false,
  onClick,
}: {
  icon: ComponentType<{ className?: string }>
  label: string
  active?: boolean
  /** Ghost mode: no border/bg, minimal hover — for secondary/utility actions. */
  ghost?: boolean
  variant?: 'default' | 'update'
  iconClassName?: string
  disabled?: boolean
  onClick: () => void
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          type="button"
          onClick={onClick}
          disabled={disabled}
          data-window-no-drag="true"
          className={cn(
            'window-no-drag flex size-11 items-center justify-center rounded-xl transition-all duration-150',
            variant === 'update'
              ? 'border border-jade/35 bg-jade text-white shadow-[0_8px_20px_rgba(28,164,107,0.26),0_0_0_0.5px_rgba(255,255,255,0.28)_inset] hover:bg-jade-dim hover:shadow-[0_10px_24px_rgba(28,164,107,0.34),0_0_0_0.5px_rgba(255,255,255,0.34)_inset]'
              : ghost
              ? 'text-muted-foreground/55 hover:text-foreground/70'
              : [
                  'border',
                  active
                    ? 'border-jade/35 bg-celadon-light/60 text-jade-dim shadow-[0_2px_8px_rgba(0,0,0,0.10),0_0_0_0.5px_rgba(0,0,0,0.06)]'
                    : 'border-border/50 bg-surface text-muted-foreground hover:border-black/[0.09] hover:bg-surface-raised hover:text-foreground/80 hover:shadow-[0_2px_10px_rgba(0,0,0,0.10),0_1px_3px_rgba(0,0,0,0.07),0_0_0_0.5px_rgba(0,0,0,0.05)]',
                ].join(' '),
            disabled && 'cursor-default opacity-75'
          )}
          aria-label={label}
        >
          <Icon className={cn('h-5 w-5', iconClassName)} />
        </button>
      </TooltipTrigger>
      <TooltipContent
        side="right"
        align="center"
        sideOffset={8}
        className="rounded-lg border border-border bg-surface-raised/90 px-2.5 py-1 text-[12px] font-medium text-foreground/85 shadow-token-md backdrop-blur"
      >
        {label}
      </TooltipContent>
    </Tooltip>
  )
}
