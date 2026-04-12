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
        'overflow-hidden rounded-[24px] border border-black/5 bg-white/66 shadow-[0_16px_40px_rgba(15,23,42,0.05)] backdrop-blur-[2px]',
        className,
      )}
    >
      {children}
    </section>
  )
}
