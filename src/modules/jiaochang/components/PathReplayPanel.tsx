import { Pause, Play, SkipBack, SkipForward } from 'lucide-react'
import type { ReactNode } from 'react'

import { Button } from '@/components/ui/button'

import type { JiaochangPathReplay, JiaochangPathReplayStep, JiaochangRuntimeAnchor } from '../data/jiaochang-types.ts'
import type { JiaochangI18nKey } from '../i18n'

export function PathReplayPanel({
  replay,
  selectedStepId,
  playing,
  onSelectStep,
  onTogglePlaying,
  onPrevious,
  onNext,
  onOpenAnchor,
  t,
}: {
  replay: JiaochangPathReplay
  selectedStepId: string | null
  playing: boolean
  onSelectStep: (stepId: string) => void
  onTogglePlaying: () => void
  onPrevious: () => void
  onNext: () => void
  onOpenAnchor: (anchor: JiaochangRuntimeAnchor) => void
  t: (key: JiaochangI18nKey) => string
}) {
  const selectedIndex = Math.max(0, replay.steps.findIndex((step) => step.id === selectedStepId))

  return (
    <section className="rounded-[8px] border border-[var(--jiaochang-border,rgba(43,34,24,0.1))] bg-[var(--jiaochang-panel-bg,rgba(255,255,255,0.7))] p-4 shadow-[var(--shadow-sm)] backdrop-blur">
      <div className="flex items-center justify-between gap-3">
        <div>
          <h2 className="font-jiaochang-pixel text-[14px] font-semibold text-[var(--jiaochang-text,#2b2218)]">
            {t('replay.title')}
          </h2>
          <p className="mt-1 text-[12px] leading-5 text-[var(--jiaochang-muted,#665a4d)]">
            {t('replay.subtitle')}
          </p>
        </div>
        <div className="flex items-center gap-1">
          <IconButton label={t('replay.previous')} onClick={onPrevious}>
            <SkipBack className="h-3.5 w-3.5" />
          </IconButton>
          <IconButton label={playing ? t('replay.pause') : t('replay.play')} onClick={onTogglePlaying}>
            {playing ? <Pause className="h-3.5 w-3.5" /> : <Play className="h-3.5 w-3.5" />}
          </IconButton>
          <IconButton label={t('replay.next')} onClick={onNext}>
            <SkipForward className="h-3.5 w-3.5" />
          </IconButton>
        </div>
      </div>

      <div className="mt-3 grid gap-2 md:grid-cols-4">
        {replay.steps.map((step, index) => (
          <ReplayStepCard
            key={step.id}
            step={step}
            index={index}
            active={selectedStepId === step.id || (!selectedStepId && index === selectedIndex)}
            onSelectStep={onSelectStep}
            onOpenAnchor={onOpenAnchor}
            t={t}
          />
        ))}
      </div>
    </section>
  )
}

function ReplayStepCard({
  step,
  index,
  active,
  onSelectStep,
  onOpenAnchor,
  t,
}: {
  step: JiaochangPathReplayStep
  index: number
  active: boolean
  onSelectStep: (stepId: string) => void
  onOpenAnchor: (anchor: JiaochangRuntimeAnchor) => void
  t: (key: JiaochangI18nKey) => string
}) {
  return (
    <article
      className={`rounded-[6px] border p-3 transition-colors ${
        active
          ? 'border-[#b9462f]/45 bg-[#fff1d7]'
          : 'border-[var(--jiaochang-border,rgba(43,34,24,0.08))] bg-[var(--jiaochang-card-bg,#fffaf0)]'
      }`}
    >
      <button
        type="button"
        className="block w-full text-left"
        onClick={() => onSelectStep(step.id)}
      >
        <div className="flex items-center justify-between gap-2">
          <span className="font-jiaochang-pixel text-[11px] text-[var(--jiaochang-accent,#9b3d2d)]">
            {String(index + 1).padStart(2, '0')} · {step.ts}
          </span>
          <span className="font-jiaochang-pixel rounded-full bg-white/70 px-2 py-0.5 text-[10px] text-[var(--jiaochang-muted,#7a5c3b)]">
            {t(`status.${step.status}`)}
          </span>
        </div>
        <div className="mt-2 truncate text-[12px] font-semibold text-[var(--jiaochang-text,#2b2218)]">
          {step.title}
        </div>
      </button>
      {step.anchor ? (
        <button
          type="button"
          className="font-jiaochang-pixel mt-2 text-[11px] font-semibold text-[#8b3a28] hover:underline"
          onClick={() => onOpenAnchor(step.anchor!)}
        >
          {t('anchor.open')} · {step.anchor.label}
        </button>
      ) : null}
    </article>
  )
}

function IconButton({
  label,
  onClick,
  children,
}: {
  label: string
  onClick: () => void
  children: ReactNode
}) {
  return (
    <Button
      type="button"
      variant="outline"
      size="icon"
      aria-label={label}
      title={label}
      onClick={onClick}
      className="size-8 rounded-[6px] border-[var(--jiaochang-border,rgba(43,34,24,0.15))] bg-white/70"
    >
      {children}
    </Button>
  )
}
