import { type MouseEvent as ReactMouseEvent, useEffect, useState } from 'react'
import { WaveDotsAnimation } from './WaveDotsAnimation'

type If2AiLoadingScreenProps = {
  projectName?: string
  stageLabel?: string
  onWindowDrag?: (event: ReactMouseEvent<HTMLElement>) => void
}

const appLogoSrc = `${new URL('../../../src-tauri/icons/icon-512.png', import.meta.url).href}?v=20260414d`
const APP_NAME = 'UClaw'

export function If2AiLoadingScreen({
  projectName = APP_NAME,
  stageLabel = 'Initializing agent workspace',
  onWindowDrag,
}: If2AiLoadingScreenProps) {
  const [buildCode, setBuildCode] = useState('ZQ4L-N97J')
  const [typedLength, setTypedLength] = useState(0)
  const [phase, setPhase] = useState(0)

  useEffect(() => {
    setBuildCode(Math.random().toString(36).slice(2, 10).toUpperCase())
  }, [])

  useEffect(() => {
    let frameId = 0
    const startedAt = performance.now()

    const tick = () => {
      const elapsed = (performance.now() - startedAt) / 1000
      setPhase(elapsed)
      frameId = window.requestAnimationFrame(tick)
    }

    frameId = window.requestAnimationFrame(tick)
    return () => window.cancelAnimationFrame(frameId)
  }, [])

  useEffect(() => {
    setTypedLength(0)
    const timeoutIds: number[] = []

    Array.from(projectName).forEach((_, index) => {
      const nextDelay = 220 + index * 110 + Math.random() * 36
      const timeoutId = window.setTimeout(() => {
        setTypedLength(index + 1)
      }, nextDelay)
      timeoutIds.push(timeoutId)
    })

    return () => {
      timeoutIds.forEach((timeoutId) => window.clearTimeout(timeoutId))
    }
  }, [projectName])

  const visibleTitle = projectName.slice(0, typedLength)
  const typingComplete = typedLength >= projectName.length
  const displayTitle = typingComplete ? projectName : visibleTitle
  const flickerOpacity = 0.94 + 0.06 * (0.5 + 0.5 * Math.sin(phase * 18))
  const titleLift = Math.sin(phase * 1.35) * 1.2

  return (
    <div
      className="relative flex min-h-screen cursor-default items-center justify-center overflow-hidden bg-sand text-ink"
      onMouseDown={onWindowDrag}
    >
      <div className="absolute inset-0 bg-[radial-gradient(circle_at_50%_36%,rgba(255,255,255,0.66),transparent_26%),radial-gradient(circle_at_50%_50%,var(--brand-orange-light)/6,transparent_48%),linear-gradient(180deg,var(--sand-warm)_0%,var(--sand)_100%)]" />
      <StartupGrid phase={phase} />
      <div className="absolute inset-0 opacity-35">
        <div className="absolute left-0 right-0 top-1/2 h-px -translate-y-1/2 bg-gradient-to-r from-transparent via-brand-orange/16 to-transparent" />
        <div className="absolute bottom-0 top-0 left-1/2 w-px -translate-x-1/2 bg-gradient-to-b from-transparent via-brand-orange/12 to-transparent" />
      </div>
      <div className="absolute right-6 top-6 z-10 flex items-center gap-3 text-[12px] font-medium tracking-[0.08em] text-black/18">
        <span>{buildCode}</span>
        <span>•</span>
        <span>v1.0.1</span>
      </div>

      <div className="relative z-10 flex w-full max-w-5xl flex-col items-center px-6 py-12 text-center">
        <div className="flex min-h-[70vh] flex-col items-center justify-center">
          <div className="relative">
            <div className="absolute inset-0 rounded-[6px] bg-[radial-gradient(circle_at_50%_50%,var(--brand-orange-light)/18,transparent_62%)] blur-2xl" />
            <div className="relative overflow-hidden rounded-[6px] border border-white/80 bg-white/64 p-1 shadow-[0_10px_40px_var(--brand-orange)/8,0_0_0_1px_rgba(255,255,255,0.66)] backdrop-blur-xl">
              <img
                src={appLogoSrc}
                alt={`${projectName} logo`}
                className="h-[110px] w-[110px] rounded-[6px] object-cover"
                draggable={false}
              />
            </div>
          </div>

          <div className="mt-14">
            <div className="relative inline-flex items-center justify-center">
              {typingComplete ? (
                <span
                  aria-hidden="true"
                  className="pointer-events-none absolute inset-0 text-[76px] font-black leading-none tracking-[-0.08em] text-brand-orange/40 blur-[10px] sm:text-[88px]"
                  style={{ transform: `translateY(${titleLift * 0.45}px) scale(1.02)` }}
                >
                  {projectName}
                </span>
              ) : null}
              <h1
                className={[
                  'loading-title-main loading-title-wordmark relative text-[76px] font-bold leading-none tracking-[-0.09em] sm:text-[88px]',
                  typingComplete ? 'loading-glitch-base' : '',
                ].join(' ')}
                data-text={projectName}
                style={{
                  opacity: flickerOpacity,
                  transform: `translateY(${titleLift}px)`,
                }}
              >
                <span className="relative z-10 bg-[linear-gradient(180deg,var(--brand-orange-light)_0%,var(--brand-orange-blend)_46%,var(--brand-orange-dark)_100%)] bg-clip-text text-transparent drop-shadow-[0_10px_18px_var(--brand-orange-blend)/18]">
                  {displayTitle}
                  <span
                    className={[
                      'ml-1 inline-block h-[0.9em] w-[0.08em] translate-y-[0.08em] rounded-full bg-brand-orange-blend align-baseline',
                      typingComplete ? 'animate-none opacity-0' : 'loading-caret',
                    ].join(' ')}
                  />
                </span>
              </h1>
              {typingComplete ? (
                <>
                  <span
                    aria-hidden="true"
                    className="loading-glitch-layer loading-glitch-layer-a loading-title-wordmark absolute inset-0 text-[76px] font-bold leading-none tracking-[-0.09em] text-brand-orange-glow sm:text-[88px]"
                  >
                    {projectName}
                  </span>
                  <span
                    aria-hidden="true"
                    className="loading-glitch-layer loading-glitch-layer-b loading-title-wordmark absolute inset-0 text-[76px] font-bold leading-none tracking-[-0.09em] text-brand-orange-dark sm:text-[88px]"
                  >
                    {projectName}
                  </span>
                </>
              ) : null}
            </div>
          </div>

          <div className="mt-10">
            <WaveDotsAnimation
              amplitude={28}
              ballRadius={4}
              count={10}
              delay={0.18}
              horizontalStretch={1.5}
              topStartColor="var(--jade-dim)"
              topEndColor="var(--brand-orange-dark)"
              bottomStartColor="var(--brand-orange-dark)"
              bottomEndColor="var(--jade-dim)"
              className="scale-[1.08]"
            />
          </div>

          <p className="mt-8 text-[14px] font-medium tracking-[0.03em] text-black/36">
            {stageLabel}
          </p>
        </div>
      </div>
    </div>
  )
}

function StartupGrid({ phase }: { phase: number }) {
  const spacing = 56
  const drift = ((phase * 0.4) % 1) * spacing

  return (
    <div className="absolute inset-0 overflow-hidden opacity-70">
      <div
        className="absolute"
        style={{
          inset: `-${spacing}px`,
          backgroundImage: `
            linear-gradient(var(--brand-orange)/8 1px, transparent 1px),
            linear-gradient(90deg, var(--brand-orange)/8 1px, transparent 1px)
          `,
          backgroundSize: `${spacing}px ${spacing}px`,
          backgroundPosition: `${drift}px 0, ${drift}px 0`,
        }}
      />
    </div>
  )
}
