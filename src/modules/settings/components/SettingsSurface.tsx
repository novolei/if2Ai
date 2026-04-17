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
        'overflow-hidden rounded-2xl border border-black/[0.07] bg-white',
        className,
      )}
      style={{ boxShadow: '0 1px 8px rgba(0,0,0,0.06), 0 0 0 0.5px rgba(0,0,0,0.03)' }}
    >
      {children}
    </section>
  )
}
