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
  iconMotion = 'none',
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
  iconMotion?: 'none' | 'vertical-loop'
  disabled?: boolean
  onClick: () => void
}) {
  const iconNode =
    iconMotion === 'vertical-loop' ? (
      <span className="relative h-5 w-5 overflow-hidden">
        <span className="absolute inset-x-0 top-0 flex flex-col items-center gap-2 motion-safe:animate-[if2ai-updater-arrow-loop_0.86s_cubic-bezier(0.65,0,0.35,1)_infinite]">
          <Icon className={cn('h-5 w-5', iconClassName)} />
          <Icon className={cn('h-5 w-5', iconClassName)} />
        </span>
      </span>
    ) : (
      <Icon className={cn('h-5 w-5', iconClassName)} />
    )

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
              ? 'border border-[#28d976]/40 bg-[#27c56b] text-white shadow-[0_8px_22px_rgba(33,197,107,0.30),0_0_0_0.5px_rgba(255,255,255,0.30)_inset] hover:bg-[#20b85f] hover:shadow-[0_10px_26px_rgba(33,197,107,0.38),0_0_0_0.5px_rgba(255,255,255,0.36)_inset]'
              : ghost
              ? 'text-muted-foreground/55 hover:text-foreground/70'
              : [
                  'border',
                  active
                    ? 'border-jade/35 bg-celadon-light/60 text-jade-dim shadow-[0_2px_8px_rgba(0,0,0,0.10),0_0_0_0.5px_rgba(0,0,0,0.06)]'
                    : 'border-border/50 bg-surface text-muted-foreground hover:border-black/[0.09] hover:bg-surface-raised hover:text-foreground/80 hover:shadow-[0_2px_10px_rgba(0,0,0,0.10),0_1px_3px_rgba(0,0,0,0.07),0_0_0_0.5px_rgba(0,0,0,0.05)]',
                ].join(' '),
            disabled && 'cursor-default opacity-95'
          )}
          aria-label={label}
        >
          {iconNode}
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
