import type { ReactNode } from 'react'
import { cn } from '@/lib/utils'

interface SettingsRowProps {
  title: string
  description?: string
  children: ReactNode
  className?: string
  /** When true, renders as a borderless row inside a surface (no individual card border) */
  inline?: boolean
}

export function SettingsRow({ title, description, children, className, inline }: SettingsRowProps) {
  if (inline) {
    return (
      <div
        className={cn(
          'flex items-center justify-between gap-4 py-2.5',
          className,
        )}
      >
        <div className="min-w-0 flex-1 space-y-0.5">
          <div className="text-[13px] font-medium tracking-tight">{title}</div>
          {description ? (
            <div className="text-[11.5px] leading-4 text-muted-foreground">{description}</div>
          ) : null}
        </div>
        <div className="shrink-0">{children}</div>
      </div>
    )
  }

  return (
    <div
      className={cn(
        'flex flex-col gap-3 rounded-xl border border-border/60 bg-muted/30 px-4 py-3 lg:flex-row lg:items-center lg:justify-between',
        className,
      )}
    >
      <div className="min-w-0 space-y-0.5">
        <div className="text-[13px] font-medium tracking-tight">{title}</div>
        {description ? (
          <div className="text-[11.5px] leading-4 text-muted-foreground">{description}</div>
        ) : null}
      </div>
      <div className="min-w-0 lg:min-w-[200px] lg:max-w-[220px]">{children}</div>
    </div>
  )
}
