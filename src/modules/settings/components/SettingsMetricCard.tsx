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
        'group relative rounded-2xl border border-border/50 bg-surface-raised px-5 py-4 shadow-token-xs transition-all hover:shadow-token-sm hover:border-primary/20',
        className,
      )}
    >
      <div className="flex items-center justify-between gap-4">
        <div className="space-y-1">
          <div className="text-[12px] text-muted-foreground">{label}</div>
          <div className="text-[24px] font-semibold tracking-tight">{value}</div>
        </div>
        <div className="flex size-10 shrink-0 items-center justify-center rounded-2xl bg-primary/10 text-primary transition-colors group-hover:bg-primary/15">
          <Icon className="h-5 w-5" />
        </div>
      </div>
      {detail ? <div className="mt-2.5 text-[12px] leading-5 text-muted-foreground">{detail}</div> : null}
    </div>
  )
}
