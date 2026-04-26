import type { ComponentType } from 'react'
import { cn } from '@/lib/utils'

interface SettingsMetricCardProps {
  icon: ComponentType<{ className?: string }>
  label: string
  value: string
  detail?: string
  className?: string
}

export function SettingsMetricCard({ icon: Icon, label, value, detail, className }: SettingsMetricCardProps) {
  return (
    <div
      className={cn(
        'group relative overflow-hidden rounded-2xl border border-border/70 bg-card px-5 py-4 text-card-foreground transition-all',
        className,
      )}
      style={{ boxShadow: 'var(--shadow-sm)' }}
    >
      <div className="flex items-start justify-between gap-3">
        <div className="space-y-1">
          <div className="text-[10.5px] font-semibold uppercase tracking-widest text-muted-foreground/70">{label}</div>
          <div className="text-[26px] font-semibold leading-none tracking-tight tabular-nums">{value}</div>
        </div>
        <div className="flex size-9 shrink-0 items-center justify-center rounded-xl bg-jade/10 text-jade transition-colors group-hover:bg-jade/15">
          <Icon className="h-4 w-4" />
        </div>
      </div>
      {detail ? (
        <div className="mt-2.5 text-[11.5px] leading-4 text-muted-foreground">{detail}</div>
      ) : null}
      <div className="pointer-events-none absolute bottom-0 right-0 h-16 w-16 rounded-full bg-jade/[0.06] blur-2xl" />
    </div>
  )
}
