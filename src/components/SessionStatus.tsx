import { cn } from '@/lib/utils'
import { Loader2, CircleCheckBig, CircleAlert, CircleDotDashed } from 'lucide-react'
import { Badge } from '@/components/ui/badge'

export type SessionStatus = 'idle' | 'running' | 'working' | 'error'

export interface SessionStatusProps {
  status: SessionStatus
  label?: string
  className?: string
}

const statusConfig = {
  idle: {
    label: '待机',
    icon: CircleDotDashed,
    className: 'text-muted-foreground bg-muted/50',
  },
  running: {
    label: '执行中',
    icon: Loader2,
    className: 'text-blue-600 bg-blue-500/10',
    animate: true,
  },
  working: {
    label: '思考中',
    icon: CircleCheckBig,
    className: 'text-emerald-600 bg-emerald-500/10',
  },
  error: {
    label: '异常',
    icon: CircleAlert,
    className: 'text-destructive bg-destructive/10',
  },
} as const

export function SessionStatus({ status, label, className }: SessionStatusProps) {
  const config = statusConfig[status]
  const Icon = config.icon

  return (
    <Badge variant="outline" className={cn('gap-1.5 rounded-full px-3 py-1 text-xs font-medium', config.className, className)}>
      <Icon className={cn('h-3.5 w-3.5', config.animate && 'animate-spin')} />
      <span>{label || config.label}</span>
    </Badge>
  )
}
