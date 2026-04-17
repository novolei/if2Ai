import type { ComponentType } from 'react'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import { cn } from '@/lib/utils'

export function NavTooltipButton({
  icon: Icon,
  label,
  active = false,
  ghost = false,
  onClick,
}: {
  icon: ComponentType<{ className?: string }>
  label: string
  active?: boolean
  /** Ghost mode: no border/bg, minimal hover — for secondary/utility actions. */
  ghost?: boolean
  onClick: () => void
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          type="button"
          onClick={onClick}
          data-window-no-drag="true"
          className={cn(
            'window-no-drag flex size-11 items-center justify-center rounded-xl transition-all duration-150',
            ghost
              ? 'text-muted-foreground/55 hover:text-foreground/70'
              : [
                  'border',
                  active
                    ? 'border-jade/35 bg-celadon-light/60 text-jade-dim shadow-[0_2px_8px_rgba(0,0,0,0.10),0_0_0_0.5px_rgba(0,0,0,0.06)]'
                    : 'border-border/50 bg-surface text-muted-foreground hover:border-black/[0.09] hover:bg-surface-raised hover:text-foreground/80 hover:shadow-[0_2px_10px_rgba(0,0,0,0.10),0_1px_3px_rgba(0,0,0,0.07),0_0_0_0.5px_rgba(0,0,0,0.05)]',
                ].join(' ')
          )}
          aria-label={label}
        >
          <Icon className="h-5 w-5" />
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
