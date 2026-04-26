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
        'overflow-hidden rounded-2xl border border-border/70 bg-card text-card-foreground',
        className,
      )}
      style={{ boxShadow: 'var(--shadow-sm)' }}
    >
      {children}
    </section>
  )
}
