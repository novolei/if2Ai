interface TypingIndicatorProps {
  dotCount?: number
  dotSize?: number
  color?: string
}

export function TypingIndicator({
  dotCount = 3,
  dotSize = 6,
  color = 'var(--color-primary)',
}: TypingIndicatorProps) {
  return (
    <div className="flex items-center gap-1">
      {Array.from({ length: dotCount }).map((_, i) => (
        <div
          key={i}
          className="animate-dot-bounce"
          style={{
            width: dotSize,
            height: dotSize,
            borderRadius: '50%',
            backgroundColor: color,
            animationDelay: `${i * 0.16}s`,
          }}
        />
      ))}
    </div>
  )
}
