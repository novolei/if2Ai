/**
 * Three pulsing dots — UClaw `DotsLoadingView` analogue.
 */
import { useEffect, useState } from 'react'

interface DotsLoadingProps {
  visible: boolean
  color?: string
  dotSize?: number
  spacing?: number
}

export function DotsLoading({
  visible,
  color = 'rgb(255,89,26)',
  dotSize = 7,
  spacing = 8,
}: DotsLoadingProps) {
  const [step, setStep] = useState(0)
  useEffect(() => {
    if (!visible) return
    const id = window.setInterval(() => setStep((s) => (s + 1) % 3), 220)
    return () => window.clearInterval(id)
  }, [visible])

  return (
    <div
      className="flex items-center justify-center"
      style={{ height: 18, opacity: visible ? 1 : 0, transition: 'opacity 200ms ease' }}
    >
      <div className="flex" style={{ gap: spacing }}>
        {[0, 1, 2].map((i) => (
          <div
            key={i}
            className="rounded-full"
            style={{
              width: dotSize,
              height: dotSize,
              background: color,
              opacity: i === step ? 1 : 0.35,
              transform: `scale(${i === step ? 1.1 : 0.85})`,
              transition: 'opacity 180ms ease, transform 180ms ease',
            }}
          />
        ))}
      </div>
    </div>
  )
}
