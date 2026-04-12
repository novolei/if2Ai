import { useEffect, useState } from 'react'
import { cn } from '@/lib/utils'

export type OrbStatus = 'idle' | 'thinking' | 'running' | 'completed' | 'error'

interface AgentOrbProps {
  status?: OrbStatus
  size?: 'sm' | 'md' | 'lg' | 'hero'
  showLabel?: boolean
  label?: string
}

const sizeMap = {
  sm: 24,
  md: 48,
  lg: 80,
  hero: 160,
}

const statusGlowColors = {
  idle: 'rgba(123, 157, 145, 0.42)',
  thinking: 'rgba(170, 186, 219, 0.42)',
  running: 'rgba(112, 185, 205, 0.42)',
  completed: 'rgba(111, 194, 156, 0.42)',
  error: 'rgba(219, 110, 127, 0.42)',
}

const statusLabels = {
  idle: '待机',
  thinking: '思考中',
  running: '执行中',
  completed: '完成',
  error: '异常',
}

export function AgentOrb({
  status = 'idle',
  size = 'md',
  showLabel = false,
  label,
}: AgentOrbProps) {
  const [animationState, setAnimationState] = useState<'breathe' | 'drift' | 'pulse'>('breathe')

  useEffect(() => {
    switch (status) {
      case 'thinking':
      case 'completed':
        setAnimationState('pulse')
        break
      case 'running':
        setAnimationState('drift')
        break
      default:
        setAnimationState('breathe')
    }
  }, [status])

  const dimension = sizeMap[size]
  const glowColor = statusGlowColors[status]

  return (
    <div className="flex flex-col items-center gap-2">
      <div
        className="relative rounded-full"
        style={{ width: dimension, height: dimension }}
      >
        <div
          className={cn(
            'absolute inset-0 rounded-full border border-white/60 bg-white/30 shadow-[0_0_60px_rgba(255,255,255,0.18)] backdrop-blur-xl',
            animationState === 'breathe' && 'animate-[pulse_6s_ease-in-out_infinite]'
          )}
          style={{
            background:
              'radial-gradient(circle at 35% 30%, rgba(255,255,255,0.92), rgba(255,255,255,0.24) 35%, transparent 60%)',
          }}
        />

        <div
          className={cn(
            'absolute inset-2 rounded-full',
            animationState === 'drift' && 'animate-[pulse_4.5s_ease-in-out_infinite]'
          )}
          style={{
            background: `radial-gradient(circle at 30% 30%, ${glowColor}, transparent 70%)`,
            opacity: 0.9,
            filter: 'blur(4px)',
          }}
        />

        <div
          className={cn(
            'absolute inset-4 rounded-full',
            animationState === 'pulse' && 'animate-[pulse_3.2s_ease-in-out_infinite]'
          )}
          style={{
            background:
              'radial-gradient(circle at 40% 40%, rgba(255,255,255,0.96), rgba(255,255,255,0.28) 50%, transparent 70%)',
          }}
        />

        <div
          className="absolute inset-0 rounded-full"
          style={{
            background: `
              radial-gradient(circle at 20% 80%, rgba(220,207,244,0.15), transparent 30%),
              radial-gradient(circle at 80% 20%, rgba(143,175,214,0.15), transparent 30%),
              radial-gradient(circle at 50% 50%, rgba(217,236,229,0.1), transparent 50%)
            `,
          }}
        />

        <div
          className="absolute inset-0 rounded-full"
          style={{
            boxShadow: `0 0 ${dimension * 0.3}px ${glowColor}`,
            opacity: status === 'error' ? 0.72 : 0.45,
          }}
        />
      </div>

      {showLabel && (
        <span className="text-xs font-medium text-muted-foreground">
          {label || statusLabels[status]}
        </span>
      )}
    </div>
  )
}

export function HeroOrb() {
  return (
    <div className="relative">
      <div
        className="absolute rounded-full blur-3xl animate-[pulse_8s_ease-in-out_infinite]"
        style={{
          width: 320,
          height: 320,
          top: -80,
          left: -80,
          background: 'radial-gradient(circle, rgba(166,191,180,0.38) 0%, transparent 70%)',
        }}
      />
      <div
        className="absolute rounded-full blur-3xl animate-[pulse_10s_ease-in-out_infinite]"
        style={{
          width: 280,
          height: 280,
          top: -60,
          left: -40,
          background: 'radial-gradient(circle, rgba(139,162,207,0.28) 0%, transparent 70%)',
          animationDelay: '-4s',
        }}
      />
      <div
        className="absolute rounded-full blur-3xl animate-[pulse_12s_ease-in-out_infinite]"
        style={{
          width: 240,
          height: 240,
          top: -40,
          left: 0,
          background: 'radial-gradient(circle, rgba(210,224,219,0.34) 0%, transparent 70%)',
          animationDelay: '-8s',
        }}
      />

      <AgentOrb status="idle" size="hero" />

      <div className="absolute -bottom-8 left-1/2 -translate-x-1/2 whitespace-nowrap">
        <span className="text-sm font-medium text-muted-foreground">准备就绪</span>
      </div>
    </div>
  )
}
