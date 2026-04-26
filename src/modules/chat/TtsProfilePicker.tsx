/**
 * TtsProfilePicker — chat-side dropdown that lists available TTS
 * profiles (from `~/.if2ai/tts_profiles.json`) and lets the user
 * switch the active one with a single click.
 *
 * Replaces the legacy chat-side voice selection: voice picking lives
 * inside each profile now (Settings → TTS Profiles).
 *
 * UX:
 * - Closed state: "{profile.name}  ▾" — compact button
 * - Open state: floating list of profiles (default starred), each
 *   showing voice id + speed + quality preset.  Click → switches.
 * - Footer link → opens Settings → TTS Profiles for management.
 */
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { ChevronDown, Settings as SettingsIcon, Star } from 'lucide-react'
import {
  listTtsProfiles,
  type TtsProfile,
  type TtsProfileBook,
  type TtsQualityPreset,
} from '@/lib/tauri'
import {
  getActiveProfileId,
  setActiveProfileId,
} from './activeTtsProfile'
import { useCrossWindowChange } from '@/lib/crossWindowSync'
import { openSettingsWindow } from '@/lib/tauri'
import { cn } from '@/lib/utils'

const QUALITY_LABEL: Record<TtsQualityPreset, string> = {
  natural: '自然',
  balanced: '平衡',
  precise: '精确',
}

interface Props {
  className?: string
  /** When true, render only the icon — useful in tight headers. */
  compact?: boolean
}

export function TtsProfilePicker({ className = '', compact = false }: Props) {
  const [book, setBook] = useState<TtsProfileBook | null>(null)
  const [activeId, setActiveIdState] = useState<string | null>(getActiveProfileId())
  const [open, setOpen] = useState(false)
  const wrapRef = useRef<HTMLDivElement | null>(null)

  const refresh = useCallback(() => {
    void listTtsProfiles()
      .then((b) => setBook(b))
      .catch((err) => console.warn('[profile-picker] list failed', err))
  }, [])
  useEffect(refresh, [refresh])
  useCrossWindowChange<{ id?: string | null }>('cross:tts-profiles-changed', refresh)
  useCrossWindowChange<{ id?: string | null }>('cross:tts-active-profile-changed', (p) => {
    setActiveIdState(p?.id ?? getActiveProfileId())
  })

  // Click-outside to close.
  useEffect(() => {
    if (!open) return
    const onDoc = (e: MouseEvent) => {
      if (!wrapRef.current?.contains(e.target as Node)) setOpen(false)
    }
    document.addEventListener('mousedown', onDoc)
    return () => document.removeEventListener('mousedown', onDoc)
  }, [open])

  const active = useMemo<TtsProfile | null>(() => {
    if (!book) return null
    const want = activeId ?? book.default_profile_id
    return book.profiles.find((p) => p.id === want) ?? book.profiles[0] ?? null
  }, [book, activeId])

  const select = (p: TtsProfile) => {
    setActiveProfileId(p.id)
    setActiveIdState(p.id)
    setOpen(false)
  }

  const handleManage = () => {
    setOpen(false)
    void openSettingsWindow()
  }

  if (!book || book.profiles.length === 0 || !active) {
    return (
      <button
        type="button"
        onClick={handleManage}
        className={cn(
          'inline-flex items-center gap-1 rounded-lg border border-dashed border-border/70 bg-muted px-2.5 py-1 text-[10.5px] text-muted-foreground hover:bg-accent hover:text-accent-foreground',
          className,
        )}
        title="管理 TTS Profiles"
      >
        <SettingsIcon className="h-3 w-3" />
        {compact ? null : '配置 TTS'}
      </button>
    )
  }

  return (
    <div ref={wrapRef} className={cn('relative inline-block', className)}>
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        className="inline-flex max-w-[180px] items-center gap-1 rounded-lg border border-border/70 bg-muted px-2 py-1 text-[10.5px] text-popover-foreground hover:bg-accent hover:text-accent-foreground"
        title={`语音 Profile: ${active.name} · ${active.voice_id} · ${active.playback_rate.toFixed(2)}×`}
      >
        <span className="shrink-0 text-muted-foreground/60">语音:</span>
        <span className="truncate font-medium text-foreground/85">{active.name}</span>
        <span className="font-mono text-[9.5px] text-muted-foreground">
          {active.playback_rate.toFixed(2)}×
        </span>
        <ChevronDown className="h-3 w-3 shrink-0 text-muted-foreground" />
      </button>
      {open ? (
        <div className="absolute bottom-full left-0 z-40 mb-1 w-[260px] overflow-hidden rounded-xl border border-border/70 bg-popover/96 text-popover-foreground shadow-lg backdrop-blur-xl">
          <div className="max-h-[300px] overflow-y-auto py-1">
            {book.profiles.map((p) => {
              const isActive = p.id === active.id
              const isDefault = p.id === book.default_profile_id
              return (
                <button
                  key={p.id}
                  type="button"
                  onClick={() => select(p)}
                  className={cn(
                    'flex w-full items-center gap-2 px-3 py-2 text-left transition-colors',
                    isActive ? 'bg-primary/12' : 'hover:bg-accent hover:text-accent-foreground',
                  )}
                >
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-1">
                      <span className="truncate text-[12px] font-medium text-foreground/90">
                        {p.name}
                      </span>
                      {isDefault ? (
                        <Star className="h-2.5 w-2.5 fill-jade text-jade" />
                      ) : null}
                    </div>
                    <div className="mt-0.5 truncate text-[10px] text-muted-foreground">
                      {p.voice_id || '(未选 voice)'} · {p.playback_rate.toFixed(2)}× ·{' '}
                      {QUALITY_LABEL[p.quality]}
                      {p.postprocess.soften_punctuation ? ' · 软化' : ''}
                      {p.postprocess.add_trailing_dots ? ' · 省略号' : ''}
                    </div>
                  </div>
                  <span
                    className={cn(
                      'rounded px-1.5 py-px text-[9px] font-semibold uppercase',
                      p.is_builtin
                        ? 'bg-amber-500/15 text-amber-500'
                        : 'bg-violet-500/15 text-violet-500',
                    )}
                  >
                    {p.is_builtin ? 'builtin' : 'user'}
                  </span>
                </button>
              )
            })}
          </div>
          <button
            type="button"
            onClick={handleManage}
            className="flex w-full items-center justify-center gap-1.5 border-t border-border/65 bg-muted/45 px-3 py-2 text-[10.5px] text-muted-foreground hover:bg-accent hover:text-accent-foreground"
          >
            <SettingsIcon className="h-3 w-3" />
            管理 Profiles
          </button>
        </div>
      ) : null}
    </div>
  )
}
