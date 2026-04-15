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
    className: 'text-[var(--status-neutral)] bg-[var(--status-neutral-bg)]',
  },
  running: {
    label: '执行中',
    icon: Loader2,
    className: 'text-[var(--status-active)] bg-[var(--status-active-bg)]',
    animate: true,
  },
  working: {
    label: '思考中',
    icon: CircleCheckBig,
    className: 'text-[var(--status-success)] bg-[var(--status-success-bg)]',
  },
  error: {
    label: '异常',
    icon: CircleAlert,
    className: 'text-[var(--status-error)] bg-[var(--status-error-bg)]',
  },
} as const

type SessionStatusConfig = (typeof statusConfig)[SessionStatus] & {
  animate?: boolean
}

export function SessionStatus({ status, label, className }: SessionStatusProps) {
  const config = statusConfig[status] as SessionStatusConfig
  const Icon = config.icon

  return (
    <Badge variant="outline" className={cn('gap-1.5 rounded-full px-3 py-1 text-xs font-medium', config.className, className)}>
      <Icon className={cn('h-3.5 w-3.5', config.animate && 'animate-spin')} />
      <span>{label || config.label}</span>
    </Badge>
  )
}
