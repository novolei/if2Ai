import { useState, useEffect } from 'react'

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

export function AgentOrb({
  status = 'idle',
  size = 'md',
  showLabel = false,
  label,
}: AgentOrbProps) {
  const [animationState, setAnimationState] = useState<'breathe' | 'drift' | 'pulse'>('breathe')

  useEffect(() => {
    switch (status) {
      case 'idle':
        setAnimationState('breathe')
        break
      case 'thinking':
        setAnimationState('pulse')
        break
      case 'running':
        setAnimationState('drift')
        break
      case 'completed':
        setAnimationState('pulse')
        break
      case 'error':
        setAnimationState('breathe')
        break
      default:
        setAnimationState('breathe')
    }
  }, [status])

  const dimension = sizeMap[size]

  const getStatusColor = () => {
    switch (status) {
      case 'idle':
        return 'var(--color-primary)'
      case 'thinking':
        return 'var(--color-accent-mint)'
      case 'running':
        return 'var(--color-accent-cyan)'
      case 'completed':
        return 'var(--color-state-success)'
      case 'error':
        return 'var(--color-state-error)'
      default:
        return 'var(--color-primary)'
    }
  }

  return (
    <div className="flex flex-col items-center gap-2">
      <div
        className="relative rounded-full"
        style={{
          width: dimension,
          height: dimension,
        }}
      >
        {/* 外层：半透明玻璃球体 */}
        <div
          className="absolute inset-0 rounded-full"
          style={{
            background: `radial-gradient(circle at 35% 30%, rgba(255,255,255,0.9), rgba(255,255,255,0.22) 35%, transparent 60%)`,
            backgroundColor: 'rgba(255, 255, 255, 0.3)',
            backdropFilter: 'blur(8px)',
            border: '1px solid rgba(255, 255, 255, 0.5)',
            animation: animationState === 'breathe' ? 'breathe 8s ease-in-out infinite' : undefined,
          }}
        />

        {/* 中层：淡蓝/淡紫/薄荷绿流动层 */}
        <div
          className="absolute inset-2 rounded-full"
          style={{
            background: `radial-gradient(circle at 30% 30%, ${getStatusColor()}, transparent 70%)`,
            opacity: 0.7,
            animation: animationState === 'drift' ? 'drift 12s ease-in-out infinite' : undefined,
            filter: 'blur(4px)',
          }}
        />

        {/* 内层：中心柔白高光 */}
        <div
          className="absolute inset-4 rounded-full"
          style={{
            background: 'radial-gradient(circle at 40% 40%, rgba(255,255,255,0.95), rgba(255,255,255,0.3) 50%, transparent 70%)',
            animation: animationState === 'pulse' ? 'pulse-soft 3s ease-in-out infinite' : undefined,
          }}
        />

        {/* 底层：极弱彩虹折射感 */}
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

        {/* 边缘光晕 */}
        <div
          className="absolute inset-0 rounded-full"
          style={{
            boxShadow: `0 0 ${dimension * 0.3}px ${getStatusColor()}40`,
            opacity: status === 'error' ? 0.6 : 0.3,
          }}
        />
      </div>

      {/* 状态标签 */}
      {showLabel && (
        <span
          className="text-xs font-medium"
          style={{
            color: 'var(--color-text-secondary)',
            fontFamily: 'var(--font-sans)',
          }}
        >
          {label || getStatusLabel(status)}
        </span>
      )}
    </div>
  )
}

function getStatusLabel(status: OrbStatus): string {
  switch (status) {
    case 'idle':
      return '待机'
    case 'thinking':
      return '思考中'
    case 'running':
      return '执行中'
    case 'completed':
      return '完成'
    case 'error':
      return '异常'
    default:
      return '待机'
  }
}

// Hero 尺寸的 Orb，用于 Splash 页面
export function HeroOrb() {
  return (
    <div className="relative">
      {/* 背景光斑 */}
      <div
        className="absolute rounded-full animate-drift"
        style={{
          width: 320,
          height: 320,
          top: -80,
          left: -80,
          background: 'radial-gradient(circle, rgba(220,207,244,0.4) 0%, transparent 70%)',
          filter: 'blur(40px)',
          animationDelay: '0s',
        }}
      />
      <div
        className="absolute rounded-full animate-drift"
        style={{
          width: 280,
          height: 280,
          top: -60,
          left: -40,
          background: 'radial-gradient(circle, rgba(143,175,214,0.35) 0%, transparent 70%)',
          filter: 'blur(35px)',
          animationDelay: '-4s',
        }}
      />
      <div
        className="absolute rounded-full animate-drift"
        style={{
          width: 240,
          height: 240,
          top: -40,
          left: 0,
          background: 'radial-gradient(circle, rgba(217,236,229,0.45) 0%, transparent 70%)',
          filter: 'blur(30px)',
          animationDelay: '-8s',
        }}
      />

      {/* 主 Orb */}
      <AgentOrb status="idle" size="hero" />

      {/* 底部标签 */}
      <div className="absolute -bottom-8 left-1/2 -translate-x-1/2 whitespace-nowrap">
        <span
          className="text-sm font-medium"
          style={{
            color: 'var(--color-text-tertiary)',
            fontFamily: 'var(--font-sans)',
          }}
        >
          准备就绪
        </span>
      </div>
    </div>
  )
}
