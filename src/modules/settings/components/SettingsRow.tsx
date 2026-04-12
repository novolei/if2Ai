import type { ReactNode } from 'react'
import { cn } from '@/lib/utils'

interface SettingsRowProps {
  title: string
  description?: string
  children: ReactNode
  className?: string
}

export function SettingsRow({ title, description, children, className }: SettingsRowProps) {
  return (
    <div
      className={cn(
        'flex flex-col gap-3.5 rounded-[18px] border border-black/5 bg-white/72 px-4 py-3.5 lg:flex-row lg:items-center lg:justify-between',
        className,
      )}
    >
      <div className="space-y-1">
        <div className="text-[14px] font-semibold tracking-tight">{title}</div>
        {description ? <div className="text-[12px] leading-5 text-muted-foreground">{description}</div> : null}
      </div>
      <div className="min-w-0 lg:min-w-[220px]">{children}</div>
    </div>
  )
}
