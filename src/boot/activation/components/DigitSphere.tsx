/**
 * Fibonacci-sphere of multi-language characters drifting around the
 * card center.  React port of UClaw's `ActivationDigitSphereView`:
 *
 * - 160 dots distributed on a unit sphere via the Fibonacci lattice.
 * - Each dot also has a random sphere position; both are scaled and
 *   the lattice position is used (UClaw kept the morph slider but
 *   pinned `t = 1`, so we skip the morph entirely).
 * - Yaw rotates with `time * rotationSpeed`, plus two slow wobbles.
 * - Perspective `260 / (260 + z)` shrinks far dots and dims them.
 * - Frozen (no rAF) when `isAnimating === false` — the modal pauses
 *   the sphere during OTP filling / verifying for legibility.
 */

import { useEffect, useMemo, useRef, useState } from 'react'

const DEFAULT_DENSITY = 160
const ROTATION_SPEED = 0.18

const DIGITS = '0123456789'.split('')
const ALL_CHARS = (
  DIGITS.join('') +
  'アイウエオカキクケコサシスセソタチツテトナニヌネノ' +
  '가나다라마바사아자차카타파하' +
  'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz' +
  'ابپتثجچحخدذرزژسشصضطظعغفقکگلمنوهی' +
  '▓░█▀▄■●◆◇★☆'
).split('')

interface Dot {
  randomX: number
  randomY: number
  randomZ: number
  char: string
}

function fibonacciSphere(index: number, count: number) {
  const goldenAngle = Math.PI * (3 - Math.sqrt(5))
  const y = 1 - (index / Math.max(count - 1, 1)) * 2
  const radius = Math.sqrt(Math.max(0, 1 - y * y))
  const theta = goldenAngle * index
  return { x: radius * Math.cos(theta), y, z: radius * Math.sin(theta) }
}

function randomDot(charSet: string[]): Dot {
  const u = Math.random()
  const v = Math.random()
  const theta = u * 2 * Math.PI
  const phi = Math.acos(2 * v - 1)
  const r = Math.cbrt(Math.random())
  return {
    randomX: r * Math.sin(phi) * Math.cos(theta),
    randomY: r * Math.sin(phi) * Math.sin(theta),
    randomZ: r * Math.cos(phi),
    char: charSet[Math.floor(Math.random() * charSet.length)] ?? '0',
  }
}

interface DigitSphereProps {
  isAnimating: boolean
  density?: number
  /** `digits` keeps it serious; `all` mixes JP/KR/EN/FA/glitch for richness. */
  charSet?: 'digits' | 'all'
}

export function DigitSphere({
  isAnimating,
  density = DEFAULT_DENSITY,
  charSet = 'all',
}: DigitSphereProps) {
  const set = charSet === 'digits' ? DIGITS : ALL_CHARS
  const dots = useMemo<Dot[]>(
    () => Array.from({ length: density }, () => randomDot(set)),
    [density, set],
  )

  const [, force] = useState(0)
  const rotationRef = useRef(0)
  const startedAtRef = useRef<number | null>(null)

  useEffect(() => {
    if (!isAnimating) return
    let raf = 0
    const loop = (t: number) => {
      if (startedAtRef.current == null) startedAtRef.current = t
      const time = (t - startedAtRef.current) / 1000
      const yaw = time * ROTATION_SPEED
      const wobble = 0.22 * Math.sin(time * 0.58) + 0.14 * Math.sin(time * 0.19)
      rotationRef.current = yaw + wobble
      force((n) => (n + 1) % 1_000_000)
      raf = requestAnimationFrame(loop)
    }
    raf = requestAnimationFrame(loop)
    return () => cancelAnimationFrame(raf)
  }, [isAnimating])

  return (
    <div className="relative h-[160px] w-full">
      <div className="absolute inset-0 flex items-center justify-center">
        <div className="relative h-[160px] w-[160px]">
          {dots.map((dot, i) => {
            const sphere = fibonacciSphere(i, dots.length)
            const sx = sphere.x * 78
            const sy = sphere.y * 78
            const sz = sphere.z * 78
            const cosR = Math.cos(rotationRef.current)
            const sinR = Math.sin(rotationRef.current)
            const rotatedX = sx * cosR - sz * sinR
            const rotatedZ = sx * sinR + sz * cosR
            const persp = 220 / (220 + rotatedZ)
            const screenX = rotatedX * persp
            const screenY = sy * persp
            const size = Math.max(8, 13 * persp)
            const alpha = Math.max(0.25, Math.min(0.95, persp))
            return (
              <span
                key={i}
                className="absolute -translate-x-1/2 -translate-y-1/2 font-mono select-none"
                style={{
                  left: `calc(50% + ${screenX}px)`,
                  top: `calc(50% + ${screenY}px)`,
                  fontSize: `${size}px`,
                  color: `rgba(20, 20, 28, ${alpha})`,
                }}
              >
                {dot.char}
              </span>
            )
          })}
        </div>
      </div>
    </div>
  )
}
