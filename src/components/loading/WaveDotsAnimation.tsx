import { useEffect, useState } from 'react'

type WaveDotsAnimationProps = {
  amplitude?: number
  ballRadius?: number
  count?: number
  delay?: number
  horizontalStretch?: number
  topStartColor?: string
  topEndColor?: string
  bottomStartColor?: string
  bottomEndColor?: string
  className?: string
}

type Direction = 'up' | 'down'

const DEFAULT_TOP_START = '#44B982'
const DEFAULT_TOP_END = '#f25717'
const DEFAULT_BOTTOM_START = '#f25717'
const DEFAULT_BOTTOM_END = '#44B982'

export function WaveDotsAnimation({
  amplitude = 48,
  ballRadius = 8,
  count = 10,
  delay = 0.2,
  horizontalStretch = 1,
  topStartColor = DEFAULT_TOP_START,
  topEndColor = DEFAULT_TOP_END,
  bottomStartColor = DEFAULT_BOTTOM_START,
  bottomEndColor = DEFAULT_BOTTOM_END,
  className,
}: WaveDotsAnimationProps) {
  const [now, setNow] = useState(() => performance.now() / 1000)

  useEffect(() => {
    let frameId = 0
    const tick = () => {
      setNow(performance.now() / 1000)
      frameId = window.requestAnimationFrame(tick)
    }
    frameId = window.requestAnimationFrame(tick)
    return () => window.cancelAnimationFrame(frameId)
  }, [])

  const dotDiameter = ballRadius * 2
  const baseGap = ballRadius * 0.9
  const baseWidth = count * dotDiameter + (count - 1) * baseGap
  const width = baseWidth * horizontalStretch
  const gap = count > 1 ? Math.max(0, (width - count * dotDiameter) / (count - 1)) : 0
  const height = amplitude + 2 * ballRadius

  return (
    <div
      className={className}
      style={{
        position: 'relative',
        width,
        height,
      }}
    >
      <WaveDotsRow
        now={now}
        count={count}
        amplitude={amplitude}
        ballRadius={ballRadius}
        gap={gap}
        delay={delay}
        startColor={topStartColor}
        endColor={topEndColor}
        direction="down"
        yOffset={amplitude / 2}
      />
      <WaveDotsRow
        now={now}
        count={count}
        amplitude={amplitude}
        ballRadius={ballRadius}
        gap={gap}
        delay={delay}
        startColor={bottomStartColor}
        endColor={bottomEndColor}
        direction="up"
        yOffset={-amplitude / 2}
      />
    </div>
  )
}

function WaveDotsRow({
  now,
  count,
  amplitude,
  ballRadius,
  gap,
  delay,
  startColor,
  endColor,
  direction,
  yOffset,
}: {
  now: number
  count: number
  amplitude: number
  ballRadius: number
  gap: number
  delay: number
  startColor: string
  endColor: string
  direction: Direction
  yOffset: number
}) {
  return (
    <div
      style={{
        position: 'absolute',
        inset: 0,
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        gap,
        transform: `translateY(${yOffset}px)`,
      }}
    >
      {Array.from({ length: count }, (_, index) => {
        const progress = phaseProgress(now, index * delay)
        const startScale = direction === 'up' ? 1 : 0.6
        const endScale = direction === 'up' ? 0.6 : 1
        const scale = mixNumber(startScale, endScale, progress)
        const offsetDirection = direction === 'up' ? 1 : -1
        const translateY = offsetDirection * amplitude * progress
        const color = mixColor(startColor, endColor, progress)

        return (
          <span
            key={`${direction}-${index}`}
            style={{
              width: ballRadius * 2,
              height: ballRadius * 2,
              borderRadius: 9999,
              backgroundColor: color,
              transform: `translateY(${translateY}px) scale(${scale})`,
              boxShadow: `0 0 ${8 + progress * 10}px ${color}33`,
              willChange: 'transform, background-color',
            }}
          />
        )
      })}
    </div>
  )
}

function phaseProgress(now: number, delay: number) {
  const elapsed = now - delay
  const wrapped = positiveModulo(elapsed, 2)
  const linear = wrapped <= 1 ? wrapped : 2 - wrapped
  return 0.5 * (1 - Math.cos(linear * Math.PI))
}

function positiveModulo(value: number, divisor: number) {
  const remainder = value % divisor
  return remainder >= 0 ? remainder : remainder + divisor
}

function mixNumber(start: number, end: number, amount: number) {
  return start + (end - start) * amount
}

function mixColor(start: string, end: string, amount: number) {
  const from = parseHexColor(start)
  const to = parseHexColor(end)
  const mixed = [0, 1, 2].map((index) =>
    Math.round(mixNumber(from[index], to[index], amount))
  )
  return `rgb(${mixed[0]} ${mixed[1]} ${mixed[2]})`
}

function parseHexColor(color: string) {
  const normalized = color.replace('#', '')
  const hex = normalized.length === 3
    ? normalized
        .split('')
        .map((char) => char + char)
        .join('')
    : normalized

  return [
    Number.parseInt(hex.slice(0, 2), 16),
    Number.parseInt(hex.slice(2, 4), 16),
    Number.parseInt(hex.slice(4, 6), 16),
  ] as const
}
