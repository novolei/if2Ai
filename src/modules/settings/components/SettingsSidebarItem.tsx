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
      className={cn(
        'window-no-drag flex h-9 w-full items-center gap-2.5 rounded-xl px-2.5 text-left transition-colors',
        active
          ? 'bg-jade/[0.09] text-jade'
          : 'text-foreground/55 hover:bg-black/[0.04] hover:text-foreground/80',
      )}
    >
      <Icon
        className={cn(
          'h-[15px] w-[15px] shrink-0 transition-colors',
          active ? 'text-jade' : 'text-foreground/35',
        )}
      />
      <span className={cn('text-[13px] tracking-tight', active ? 'font-semibold' : 'font-medium')}>
        {label}
      </span>
    </button>
  )
}
