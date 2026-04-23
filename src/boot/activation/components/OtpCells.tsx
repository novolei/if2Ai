/**
 * 8-digit OTP cells with five visual phases.
 *
 * 1:1 port of UClaw's `ActivationOTPInputView.swift`:
 *
 *   - `idle`     : empty cells, blinking caret on the next slot.
 *   - `filling`  : cells fill left-to-right; current slot's caret blinks.
 *   - `verifying`: cells merge into a single neutral box (warning stroke).
 *   - `success`  : merged box turns green, ✓ pops in.
 *   - `error`    : red flash + shake (sequence -8,8,-6,6,-3,3,0 × 55ms).
 *
 * Cells are 36×36 with 6px gap, monospaced 15px semibold for digits.
 */

import { useEffect, useRef, useState } from 'react'

import { Check, X } from 'lucide-react'

export type OtpPhase = 'idle' | 'filling' | 'verifying' | 'success' | 'error'

interface OtpCellsProps {
  code: string
  digitsCount?: number
  phase: OtpPhase
}

const SHAKE_SEQUENCE = [-8, 8, -6, 6, -3, 3, 0]

export function OtpCells({ code, digitsCount = 8, phase }: OtpCellsProps) {
  const merged = phase === 'verifying' || phase === 'success' || phase === 'error'
  const [blink, setBlink] = useState(true)
  const [shakeOffset, setShakeOffset] = useState(0)
  const [errorFlash, setErrorFlash] = useState(false)
  const [drawSuccess, setDrawSuccess] = useState(false)
  const [drawError, setDrawError] = useState(false)
  const wrapperRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const id = window.setInterval(() => setBlink((b) => !b), 650)
    return () => window.clearInterval(id)
  }, [])

  useEffect(() => {
    if (phase === 'success') {
      setDrawSuccess(false)
      setDrawError(false)
      const t = window.setTimeout(() => setDrawSuccess(true), 30)
      return () => window.clearTimeout(t)
    }
    if (phase === 'error') {
      setErrorFlash(true)
      setDrawError(false)
      let cancelled = false
      ;(async () => {
        for (const v of SHAKE_SEQUENCE) {
          if (cancelled) return
          setShakeOffset(v)
          await new Promise((r) => setTimeout(r, 55))
        }
        if (cancelled) return
        // After the shake settles, pop the ✗ glyph in (matching the
        // success animation timing) but keep the red stroke so the
        // box stays semantically "errored" until the parent advances
        // the phase.
        setDrawError(true)
        await new Promise((r) => setTimeout(r, 320))
      })()
      return () => {
        cancelled = true
      }
    }
    setErrorFlash(false)
    setShakeOffset(0)
    setDrawSuccess(false)
    setDrawError(false)
    return undefined
  }, [phase])

  const strokeColor = (current: boolean) => {
    if (errorFlash) return 'rgb(239,68,68)'
    if (phase === 'success') return 'rgb(34,197,94)'
    if (phase === 'verifying') return 'rgb(234,179,8)'
    if (current) return 'rgba(0,0,0,0.75)'
    return 'transparent'
  }
  const strokeWidth = (current: boolean) =>
    errorFlash || phase === 'success' || phase === 'verifying' || current ? 1.8 : 0.8

  return (
    <div
      ref={wrapperRef}
      className="relative inline-flex"
      style={{ transform: `translateX(${shakeOffset}px)`, transition: 'transform 55ms ease' }}
    >
      {merged ? (
        (() => {
          // Solid pill on success / error so the white glyph stays
          // legible against the orange/red striped backdrop. The
          // verifying state keeps the translucent look + amber stroke.
          const filled = phase === 'success' || phase === 'error'
          const fill =
            phase === 'success'
              ? 'rgb(34,197,94)'
              : phase === 'error'
                ? 'rgb(239,68,68)'
                : 'rgba(255,255,255,0.18)'
          return (
            <div
              className="rounded-lg"
              style={{
                width: 36,
                height: 36,
                background: fill,
                outline: filled
                  ? 'none'
                  : `${strokeWidth(false)}px solid ${strokeColor(false)}`,
                boxShadow: filled
                  ? phase === 'success'
                    ? '0 4px 14px rgba(34,197,94,0.45)'
                    : '0 4px 14px rgba(239,68,68,0.45)'
                  : 'none',
                transition: 'background 200ms ease, box-shadow 200ms ease',
              }}
            >
              {phase === 'success' && (
                <div
                  className="flex h-full w-full items-center justify-center"
                  style={{
                    color: '#ffffff',
                    opacity: drawSuccess ? 1 : 0,
                    transform: `scale(${drawSuccess ? 1 : 0.86})`,
                    transition: 'opacity 220ms ease, transform 220ms ease',
                  }}
                >
                  <Check strokeWidth={3.2} className="h-[22px] w-[22px]" />
                </div>
              )}
              {phase === 'error' && (
                <div
                  className="flex h-full w-full items-center justify-center"
                  style={{
                    color: '#ffffff',
                    opacity: drawError ? 1 : 0,
                    transform: `scale(${drawError ? 1 : 0.86})`,
                    transition: 'opacity 220ms ease, transform 220ms ease',
                  }}
                  aria-label="激活失败"
                >
                  <X strokeWidth={3.2} className="h-[22px] w-[22px]" />
                </div>
              )}
            </div>
          )
        })()
      ) : (
        <div className="flex gap-[6px]">
          {Array.from({ length: digitsCount }).map((_, i) => {
            const ch = code[i]
            const isCurrent = i === code.length && (phase === 'filling' || phase === 'idle')
            return (
              <div
                key={i}
                className="relative rounded-lg"
                style={{
                  width: 36,
                  height: 36,
                  background: 'rgba(255,255,255,0.18)',
                  outline: `${strokeWidth(isCurrent)}px solid ${strokeColor(isCurrent)}`,
                  transition: 'outline 180ms ease',
                }}
              >
                <div className="flex h-full w-full items-center justify-center">
                  {ch ? (
                    <span
                      className="font-mono font-semibold"
                      style={{
                        fontSize: 15,
                        color: 'rgba(0,0,0,0.78)',
                        transform:
                          phase === 'filling' && i < code.length ? 'scale(1)' : 'scale(0.78)',
                        transition: 'transform 220ms cubic-bezier(0.34,1.56,0.64,1)',
                      }}
                    >
                      {ch}
                    </span>
                  ) : isCurrent ? (
                    <div
                      className="rounded-sm"
                      style={{
                        width: 2,
                        height: 14,
                        background: blink ? 'rgba(0,0,0,0.9)' : 'rgba(0,0,0,0.25)',
                        transition: 'background 100ms ease',
                      }}
                    />
                  ) : null}
                </div>
              </div>
            )
          })}
        </div>
      )}
    </div>
  )
}
