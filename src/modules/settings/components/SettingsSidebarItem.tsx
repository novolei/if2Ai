import type { ComponentType } from 'react'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'

interface SettingsSidebarItemProps {
  icon: ComponentType<{ className?: string }>
  label: string
  active: boolean
  onClick: () => void
}

export function SettingsSidebarItem({
  icon: Icon,
  label,
  active,
  onClick,
}: SettingsSidebarItemProps) {
  return (
    <Button
      type="button"
      variant="ghost"
      onClick={onClick}
      className={cn(
        'window-no-drag mb-1 flex h-12 w-full items-center justify-start gap-3 rounded-[18px] px-4 text-left font-normal transition-colors',
        active
          ? 'bg-white/82 text-foreground shadow-[0_1px_2px_rgba(15,23,42,0.05)]'
          : 'text-foreground/80 hover:bg-white/58 hover:text-foreground',
      )}
    >
      <div
        className={cn(
          'flex size-9 items-center justify-center rounded-[14px] transition-colors',
          active ? 'bg-black/5 text-foreground' : 'bg-black/[0.03] text-black/55',
        )}
      >
        <Icon className="h-4 w-4" />
      </div>
      <div className="min-w-0 truncate text-[15px] font-medium tracking-tight">{label}</div>
    </Button>
  )
}
