import { cn } from '@/lib/utils';

export type SessionStatus = 'idle' | 'running' | 'working' | 'error';

export interface SessionStatusProps {
  /** Current session status */
  status: SessionStatus;
  /** Optional label to display */
  label?: string;
  /** Optional className for styling */
  className?: string;
}

const statusConfig = {
  idle: {
    icon: '○',
    color: 'text-muted-foreground',
    label: 'Idle',
  },
  running: {
    icon: '◐',
    color: 'text-blue-500',
    label: 'Running',
    animate: true,
  },
  working: {
    icon: '●',
    color: 'text-green-500',
    label: 'Working',
  },
  error: {
    icon: '✕',
    color: 'text-red-500',
    label: 'Error',
  },
};

export function SessionStatus({ status, label, className }: SessionStatusProps) {
  const config = statusConfig[status];

  return (
    <div className={cn('flex items-center gap-1.5 text-xs', className)}>
      <span
        className={cn(
          'font-mono text-sm leading-none',
          config.color,
          config.animate && 'animate-spin'
        )}
      >
        {config.icon}
      </span>
      <span className={cn('text-muted-foreground', config.color)}>
        {label || config.label}
      </span>
    </div>
  );
}
