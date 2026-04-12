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
            'window-no-drag flex size-11 items-center justify-center rounded-[14px] border transition-colors',
            active
              ? 'border-emerald-300/55 bg-emerald-50 text-emerald-700 shadow-[0_0_0_1px_rgba(16,185,129,0.06)]'
              : 'border-black/5 bg-white/68 text-black/50 hover:bg-white hover:text-black/75'
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
        className="rounded-lg border border-black/10 bg-white/95 px-2.5 py-1 text-[12px] font-medium text-black/85 shadow-[0_10px_24px_rgba(0,0,0,0.12)] backdrop-blur"
      >
        {label}
      </TooltipContent>
    </Tooltip>
  )
}
