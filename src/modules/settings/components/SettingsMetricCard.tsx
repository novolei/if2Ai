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
        'rounded-[20px] border border-black/5 bg-white/72 p-[18px] shadow-[0_8px_22px_rgba(15,23,42,0.04)]',
        className,
      )}
    >
      <div className="flex items-center justify-between gap-4">
        <div className="space-y-1">
          <div className="text-[12px] text-muted-foreground">{label}</div>
          <div className="text-[24px] font-semibold tracking-tight">{value}</div>
        </div>
        <div className="flex size-10 items-center justify-center rounded-[14px] bg-black/5 text-black/70">
          <Icon className="h-5 w-5" />
        </div>
      </div>
      {detail ? <div className="mt-2.5 text-[12px] leading-5 text-muted-foreground">{detail}</div> : null}
    </div>
  )
}
