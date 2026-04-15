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
        'window-no-drag mb-1 flex h-12 w-full items-center justify-start gap-3 rounded-2xl px-4 text-left font-normal transition-colors',
        active
          ? 'bg-surface-raised text-foreground shadow-token-xs'
          : 'text-foreground/80 hover:bg-accent hover:text-foreground',
      )}
    >
      <div
        className={cn(
          'flex size-9 items-center justify-center rounded-2xl transition-colors',
          active ? 'bg-muted text-foreground' : 'bg-muted/50 text-muted-foreground',
        )}
      >
        <Icon className="h-4 w-4" />
      </div>
      <div className="min-w-0 truncate text-[15px] font-medium tracking-tight">{label}</div>
    </Button>
  )
}
