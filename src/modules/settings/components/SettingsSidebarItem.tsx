import type { ComponentType } from 'react'
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
    <button
      type="button"
      onClick={onClick}
      aria-current={active ? 'page' : undefined}
      className={cn(
        'window-no-drag flex h-9 w-full items-center gap-2.5 rounded-xl px-2.5 text-left transition-colors',
        active
          ? 'bg-sidebar-accent text-sidebar-accent-foreground'
          : 'text-sidebar-foreground/70 hover:bg-sidebar-accent/70 hover:text-sidebar-accent-foreground',
      )}
    >
      <Icon
        className={cn(
          'h-[15px] w-[15px] shrink-0 transition-colors',
          active ? 'text-jade' : 'text-sidebar-foreground/55',
        )}
      />
      <span className={cn('text-[13px] tracking-tight', active ? 'font-semibold' : 'font-medium')}>
        {label}
      </span>
    </button>
  )
}
