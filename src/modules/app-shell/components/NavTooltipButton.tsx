import type { ComponentType } from 'react'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import { cn } from '@/lib/utils'

export function NavTooltipButton({
  icon: Icon,
  label,
  active = false,
  onClick,
}: {
  icon: ComponentType<{ className?: string }>
  label: string
  active?: boolean
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
            'window-no-drag flex size-11 items-center justify-center rounded-xl border transition-colors',
            active
              ? 'border-jade/40 bg-celadon-light/60 text-jade-dim shadow-xs'
              : 'border-border/50 bg-surface text-muted-foreground hover:bg-surface-raised hover:text-foreground/80'
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
