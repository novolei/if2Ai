import { ListMusic, Pause, Play, Plus, Repeat, Repeat1, Search, ShieldCheck, Shuffle, SkipBack, SkipForward, Trash2, Upload, Volume2, VolumeX } from 'lucide-react'
import { useEffect, useMemo, useState } from 'react'
import type { ReactNode } from 'react'

import { Button } from '@/components/ui/button'

import type { JiaochangMusicTrack, JiaochangRepeatMode } from '../audio/audio-state.ts'
import { createJiaochangVisualizerFrame, createSilentVisualizerFrame } from '../audio/audio-visualizer.ts'
import { listJiaochangAudioPluginSources, searchJiaochangAudioPluginTracks, type JiaochangAudioPluginManifest } from '../audio/plugin-source-adapter.ts'
import { createPluginTrackCandidates } from '../audio/track-library.ts'
import { useJiaochangAudio } from '../audio/useJiaochangAudio.ts'
import type { JiaochangTranslator } from '../i18n/index.ts'

const REPEAT_ORDER: JiaochangRepeatMode[] = ['off', 'all', 'one', 'shuffle']

export function JiaochangMusicPlayer({ t }: { t: JiaochangTranslator }) {
  const { state, activeTrack, controls } = useJiaochangAudio()
  const [query, setQuery] = useState('')
  const [pluginSources, setPluginSources] = useState<JiaochangAudioPluginManifest[]>([])
  const [pluginSearchResults, setPluginSearchResults] = useState<JiaochangMusicTrack[]>([])
  const visualizer = useMemo(() => {
    if (!state.isPlaying) return createSilentVisualizerFrame(18)
    return createJiaochangVisualizerFrame(Math.round(state.currentTime * 240), 18)
  }, [state.currentTime, state.isPlaying])
  const duration = state.duration > 0 ? state.duration : activeTrack?.duration ?? 0
  const seekMax = Math.max(duration, state.currentTime, 1)
  const nextRepeatMode = REPEAT_ORDER[(REPEAT_ORDER.indexOf(state.repeatMode) + 1) % REPEAT_ORDER.length] ?? 'off'
  const filteredQueue = useMemo(() => {
    const normalized = query.trim().toLowerCase()
    if (!normalized) return state.queue.slice(0, 4)
    return state.queue
      .filter((track) => `${track.title} ${track.artist ?? ''} ${track.sourceTrackId ?? ''}`.toLowerCase().includes(normalized))
      .slice(0, 4)
  }, [query, state.queue])
  const fallbackPluginCandidates = useMemo(() => {
    if (!query.trim()) return []
    return createPluginTrackCandidates(pluginSources, query).slice(0, 4)
  }, [pluginSources, query])
  const pluginCandidates = pluginSearchResults.length > 0 ? pluginSearchResults : fallbackPluginCandidates

  useEffect(() => {
    let cancelled = false
    listJiaochangAudioPluginSources()
      .then((sources) => {
        if (!cancelled) setPluginSources(sources)
      })
      .catch(() => {
        if (!cancelled) setPluginSources([])
      })
    return () => {
      cancelled = true
    }
  }, [])

  useEffect(() => {
    const normalized = query.trim()
    if (!normalized) {
      setPluginSearchResults([])
      return
    }
    const searchable = pluginSources.flatMap((plugin) => {
      if (!plugin.enabled) return []
      return (plugin.providers ?? [])
        .filter((provider) => provider.enabled && provider.actions?.some((action) => action === 'musicSearch' || action === 'search'))
        .map((provider) => ({ plugin, provider }))
    })
    if (searchable.length === 0) {
      setPluginSearchResults([])
      return
    }
    let cancelled = false
    const timer = window.setTimeout(() => {
      void Promise.allSettled(searchable.slice(0, 2).map(({ plugin, provider }) => (
        searchJiaochangAudioPluginTracks({
          plugin_id: plugin.plugin_id,
          source: provider.provider_id,
          query: normalized,
          page: 1,
          page_size: 6,
        })
      ))).then((settled) => {
        if (cancelled) return
        const tracks = settled.flatMap((item) => item.status === 'fulfilled' ? item.value : [])
          .map(pluginSearchResultToTrack)
          .slice(0, 8)
        setPluginSearchResults(tracks)
      })
    }, 350)
    return () => {
      cancelled = true
      window.clearTimeout(timer)
    }
  }, [pluginSources, query])

  return (
    <section className="rounded-[8px] border border-[var(--jiaochang-border,rgba(43,34,24,0.12))] bg-[var(--jiaochang-panel-bg,rgba(255,255,255,0.76))] p-3 shadow-[0_10px_28px_rgba(43,34,24,0.08)]">
      <div className="mb-3 flex items-start justify-between gap-3">
        <div className="min-w-0">
          <div className="font-jiaochang-pixel flex items-center gap-2 text-[13px] text-[var(--jiaochang-accent,#9b3d2d)]">
            <ListMusic className="h-4 w-4" />
            {t('audio.title')}
          </div>
          <div className="mt-1 truncate text-[13px] font-semibold text-[var(--jiaochang-text,#2b2218)]">
            {activeTrack ? activeTrack.title : t('audio.empty')}
          </div>
          <div className="mt-0.5 truncate text-[11px] text-[var(--jiaochang-muted,#6c604d)]">
            {activeTrack?.artist ?? activeTrack?.license ?? t('audio.noAutoplay')}
          </div>
        </div>
        <Button
          type="button"
          variant="outline"
          size="sm"
          onClick={() => void controls.importLocalTracksFromDialog()}
          className="font-jiaochang-pixel h-8 shrink-0 rounded-[6px] border-[var(--jiaochang-border,rgba(43,34,24,0.16))] bg-white/70 px-2 text-[11px]"
        >
          <Upload className="mr-1 h-3.5 w-3.5" />
          {t('audio.importLocal')}
        </Button>
      </div>

      <div className="mb-3 flex h-8 items-end gap-1 overflow-hidden rounded-[6px] border border-[var(--jiaochang-border,rgba(43,34,24,0.1))] bg-[rgba(236,248,229,0.72)] px-2 py-1">
        {visualizer.bars.map((bar, index) => (
          <span
            key={index}
            className="w-full min-w-[3px] rounded-t-[2px] bg-[var(--jiaochang-accent,#b44732)]"
            style={{ height: `${Math.max(10, bar * 100)}%`, opacity: 0.35 + bar * 0.55 }}
          />
        ))}
      </div>

      <div className="grid gap-2">
        <input
          type="range"
          min={0}
          max={seekMax}
          step={0.5}
          value={Math.min(state.currentTime, seekMax)}
          onChange={(event) => controls.seek(Number(event.currentTarget.value))}
          className="h-1.5 w-full accent-[var(--jiaochang-accent,#b44732)]"
          aria-label={t('audio.seek')}
          disabled={!activeTrack}
        />
        <div className="flex items-center justify-between gap-2 text-[11px] text-[var(--jiaochang-muted,#6c604d)]">
          <span>{formatTime(state.currentTime)}</span>
          <span>{formatTime(duration)}</span>
        </div>
      </div>

      <div className="mt-3 flex items-center justify-between gap-2">
        <div className="flex items-center gap-1">
          <IconButton label={t('audio.previous')} disabled={state.queue.length === 0} onClick={() => void controls.previous()}>
            <SkipBack className="h-4 w-4" />
          </IconButton>
          <IconButton
            label={state.isPlaying ? t('audio.pause') : t('audio.play')}
            disabled={state.queue.length === 0 || state.isResolving}
            onClick={() => void controls.togglePlay()}
            primary
          >
            {state.isPlaying ? <Pause className="h-4 w-4" /> : <Play className="h-4 w-4" />}
          </IconButton>
          <IconButton label={t('audio.next')} disabled={state.queue.length === 0} onClick={() => void controls.next()}>
            <SkipForward className="h-4 w-4" />
          </IconButton>
          <IconButton label={repeatLabel(t, nextRepeatMode)} onClick={() => controls.setRepeatMode(nextRepeatMode)}>
            {state.repeatMode === 'one' ? <Repeat1 className="h-4 w-4" /> : state.repeatMode === 'shuffle' ? <Shuffle className="h-4 w-4" /> : <Repeat className="h-4 w-4" />}
          </IconButton>
        </div>
        <div className="flex min-w-[112px] items-center gap-2">
          <button
            type="button"
            className="text-[var(--jiaochang-muted,#6c604d)] hover:text-[var(--jiaochang-accent,#b44732)]"
            onClick={controls.toggleMute}
            aria-label={state.muted ? t('audio.unmute') : t('audio.mute')}
          >
            {state.muted ? <VolumeX className="h-4 w-4" /> : <Volume2 className="h-4 w-4" />}
          </button>
          <input
            type="range"
            min={0}
            max={1}
            step={0.01}
            value={state.volume}
            onChange={(event) => controls.setVolume(Number(event.currentTarget.value))}
            className="w-full accent-[var(--jiaochang-accent,#b44732)]"
            aria-label={t('audio.volume')}
          />
        </div>
      </div>

      <div className="mt-3 flex items-center justify-between gap-2 text-[11px] text-[var(--jiaochang-muted,#6c604d)]">
        <span className="font-jiaochang-pixel">{t('audio.queue')}: {state.queue.length}</span>
        <span className="font-jiaochang-pixel">{repeatLabel(t, state.repeatMode)}</span>
      </div>

      <div className="mt-3 rounded-[6px] border border-[var(--jiaochang-border,rgba(43,34,24,0.12))] bg-white/50 p-2">
        <label className="flex h-8 items-center gap-2 rounded-[5px] border border-[var(--jiaochang-border,rgba(43,34,24,0.12))] bg-white/75 px-2">
          <Search className="h-3.5 w-3.5 text-[var(--jiaochang-muted,#6c604d)]" />
          <input
            value={query}
            onChange={(event) => setQuery(event.currentTarget.value)}
            placeholder={t('audio.search')}
            className="min-w-0 flex-1 bg-transparent text-[12px] text-[var(--jiaochang-text,#2b2218)] outline-none placeholder:text-[var(--jiaochang-muted,#6c604d)]/65"
          />
        </label>
        <div className="mt-2 grid gap-1.5">
          {filteredQueue.map((track) => (
            <CandidateRow
              key={track.id}
              label={track.title}
              meta={track.artist ?? track.source}
              active={track.id === state.activeTrackId}
              actionLabel={t('audio.play')}
              onClick={() => void controls.play(track.id)}
              onRemove={() => controls.removeTrackFromQueue(track.id)}
              removeLabel={t('audio.remove')}
            />
          ))}
          {pluginCandidates.map((track) => (
            <CandidateRow
              key={track.id}
              label={track.title}
              meta={`${t('audio.pluginCandidate')} · ${track.artist ?? track.sourceAdapterId}`}
              actionLabel={t('audio.add')}
              icon={<ShieldCheck className="h-3 w-3" />}
              onClick={() => controls.addTracksToQueue([track])}
            />
          ))}
          {filteredQueue.length === 0 && pluginCandidates.length === 0 ? (
            <div className="px-1 py-1 text-[11px] text-[var(--jiaochang-muted,#6c604d)]">
              {query.trim() ? t('audio.noCandidates') : t('audio.searchHint')}
            </div>
          ) : null}
        </div>
      </div>

      {state.error ? (
        <div className="mt-2 rounded-[6px] border border-[rgba(185,71,50,0.24)] bg-[rgba(185,71,50,0.08)] px-2 py-1.5 text-[11px] text-[var(--jiaochang-accent,#9b3d2d)]">
          {t('audio.error')}: {state.error.message}
        </div>
      ) : null}
    </section>
  )
}

function pluginSearchResultToTrack(result: Awaited<ReturnType<typeof searchJiaochangAudioPluginTracks>>[number]): JiaochangMusicTrack {
  return {
    id: result.track_id,
    title: result.title,
    artist: result.artist,
    album: result.album,
    duration: result.duration,
    source: 'plugin',
    sourceAdapterId: result.plugin_id,
    sourceTrackId: result.source_track_id,
    mood: 'focus',
    availableQualities: ['standard', 'high', 'lossless'],
    license: `plugin-source:${result.plugin_id}:${result.source}`,
    pluginSource: {
      pluginId: result.plugin_id,
      source: result.source,
      musicInfo: result.music_info,
    },
  }
}

function CandidateRow({
  actionLabel,
  active = false,
  icon,
  label,
  meta,
  onClick,
  onRemove,
  removeLabel,
}: {
  actionLabel: string
  active?: boolean
  icon?: ReactNode
  label: string
  meta: string
  onClick: () => void
  onRemove?: () => void
  removeLabel?: string
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`flex min-h-8 items-center gap-2 rounded-[5px] px-2 py-1 text-left transition ${
        active ? 'bg-[rgba(185,71,50,0.12)]' : 'hover:bg-white/80'
      }`}
      title={actionLabel}
    >
      <span className="min-w-0 flex-1">
        <span className="block truncate text-[12px] font-semibold text-[var(--jiaochang-text,#2b2218)]">{label}</span>
        <span className="block truncate text-[10.5px] text-[var(--jiaochang-muted,#6c604d)]">{meta}</span>
      </span>
      <span className="flex h-6 w-6 shrink-0 items-center justify-center rounded-[5px] border border-[var(--jiaochang-border,rgba(43,34,24,0.12))] bg-white/70 text-[var(--jiaochang-accent,#b44732)]">
        {icon ?? <Plus className="h-3.5 w-3.5" />}
      </span>
      {onRemove ? (
        <span
          role="button"
          tabIndex={0}
          className="flex h-6 w-6 shrink-0 items-center justify-center rounded-[5px] border border-[rgba(185,71,50,0.18)] bg-white/70 text-[var(--jiaochang-accent,#b44732)]"
          title={removeLabel}
          aria-label={removeLabel}
          onClick={(event) => {
            event.stopPropagation()
            onRemove()
          }}
          onKeyDown={(event) => {
            if (event.key !== 'Enter' && event.key !== ' ') return
            event.stopPropagation()
            onRemove()
          }}
        >
          <Trash2 className="h-3.5 w-3.5" />
        </span>
      ) : null}
    </button>
  )
}

function IconButton({
  children,
  disabled,
  label,
  onClick,
  primary = false,
}: {
  children: ReactNode
  disabled?: boolean
  label: string
  onClick: () => void
  primary?: boolean
}) {
  return (
    <button
      type="button"
      className={`flex h-8 w-8 items-center justify-center rounded-[6px] border text-[var(--jiaochang-text,#2b2218)] transition disabled:cursor-not-allowed disabled:opacity-40 ${
        primary
          ? 'border-[var(--jiaochang-accent,#b44732)] bg-[rgba(185,71,50,0.12)]'
          : 'border-[var(--jiaochang-border,rgba(43,34,24,0.14))] bg-white/70 hover:bg-white'
      }`}
      aria-label={label}
      title={label}
      disabled={disabled}
      onClick={onClick}
    >
      {children}
    </button>
  )
}

function repeatLabel(t: JiaochangTranslator, mode: JiaochangRepeatMode): string {
  switch (mode) {
    case 'one':
      return t('audio.repeat.one')
    case 'all':
      return t('audio.repeat.all')
    case 'shuffle':
      return t('audio.repeat.shuffle')
    case 'off':
      return t('audio.repeat.off')
  }
}

function formatTime(value: number): string {
  if (!Number.isFinite(value) || value <= 0) return '0:00'
  const minutes = Math.floor(value / 60)
  const seconds = Math.floor(value % 60)
  return `${minutes}:${seconds.toString().padStart(2, '0')}`
}
