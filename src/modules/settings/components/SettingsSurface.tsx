import type { ReactNode } from 'react'
import { cn } from '@/lib/utils'

interface SettingsSurfaceProps {
  children: ReactNode
  className?: string
}

export function SettingsSurface({ children, className }: SettingsSurfaceProps) {
  return (
    <section
      className={cn(
        'overflow-hidden rounded-2xl border border-border/50 bg-surface-raised shadow-token-lg backdrop-blur-[2px]',
        className,
      )}
    >
      {children}
    </section>
  )
}
