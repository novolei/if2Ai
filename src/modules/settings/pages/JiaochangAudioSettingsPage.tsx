import { useCallback, useEffect, useMemo, useState } from 'react'
import { AlertCircle, CheckCircle2, Eraser, ListMusic, Plug, RefreshCw, Save, ShieldCheck } from 'lucide-react'
import { toast } from 'sonner'

import { Button } from '@/components/ui/button'

import {
  clearJiaochangAudioPluginCache,
  getJiaochangAudioPluginCacheInfo,
  listJiaochangAudioPluginSources,
  registerJiaochangAudioPluginSource,
  subscribeJiaochangAudioPluginEvents,
  type JiaochangAudioPluginCacheInfo,
  type JiaochangAudioPluginManifest,
  type JiaochangAudioPluginRuntimeEvent,
} from '@/modules/jiaochang/audio/plugin-source-adapter'
import {
  DEFAULT_JIACHANG_PLUGIN_MANIFEST_DRAFT,
  draftToPluginManifest,
  pluginManifestToDraft,
  validatePluginManifestDraft,
  type JiaochangPluginManifestDraft,
} from '@/modules/jiaochang/audio/plugin-source-settings'

import { CompactInput } from '../components/CompactInput'
import { SettingsSurface } from '../components/SettingsSurface'

const MAX_EVENTS = 80

export function JiaochangAudioSettingsPage() {
  const [draft, setDraft] = useState<JiaochangPluginManifestDraft>(DEFAULT_JIACHANG_PLUGIN_MANIFEST_DRAFT)
  const [plugins, setPlugins] = useState<JiaochangAudioPluginManifest[]>([])
  const [cacheInfo, setCacheInfo] = useState<JiaochangAudioPluginCacheInfo>({ entries: 0 })
  const [events, setEvents] = useState<JiaochangAudioPluginRuntimeEvent[]>([])
  const [loading, setLoading] = useState(false)
  const validationError = useMemo(() => validatePluginManifestDraft(draft), [draft])

  const refresh = useCallback(async () => {
    setLoading(true)
    try {
      const [nextPlugins, nextCacheInfo] = await Promise.all([
        listJiaochangAudioPluginSources(),
        getJiaochangAudioPluginCacheInfo(),
      ])
      setPlugins(nextPlugins)
      setCacheInfo(nextCacheInfo)
    } catch (error) {
      toast.error('校场音源刷新失败', { description: String(error) })
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    void refresh()
  }, [refresh])

  useEffect(() => {
    let disposed = false
    let cleanup: (() => void) | null = null
    subscribeJiaochangAudioPluginEvents((event) => {
      setEvents((current) => [event, ...current].slice(0, MAX_EVENTS))
      if (event.event_type === 'resolved' || event.event_type === 'cache_hit') {
        void getJiaochangAudioPluginCacheInfo().then(setCacheInfo).catch(() => undefined)
      }
    })
      .then((unlisten) => {
        if (disposed) {
          unlisten()
          return
        }
        cleanup = unlisten
      })
      .catch((error) => {
        setEvents((current) => [{
          event_type: 'error',
          plugin_id: 'settings-ui',
          message: String(error),
          timestamp: new Date().toISOString(),
        }, ...current].slice(0, MAX_EVENTS))
      })
    return () => {
      disposed = true
      cleanup?.()
    }
  }, [])

  const updateDraft = <Key extends keyof JiaochangPluginManifestDraft>(
    key: Key,
    value: JiaochangPluginManifestDraft[Key],
  ) => {
    setDraft((current) => ({ ...current, [key]: value }))
  }

  const handleRegister = async () => {
    const error = validatePluginManifestDraft(draft)
    if (error) {
      toast.error('Manifest 不完整', { description: error })
      return
    }
    try {
      const registered = await registerJiaochangAudioPluginSource(draftToPluginManifest(draft))
      toast.success('插件音源已注册', { description: registered.plugin_id })
      setDraft(pluginManifestToDraft(registered))
      await refresh()
    } catch (registerError) {
      toast.error('插件音源注册失败', { description: String(registerError) })
    }
  }

  const handleClearCache = async () => {
    try {
      const nextCacheInfo = await clearJiaochangAudioPluginCache()
      setCacheInfo(nextCacheInfo)
      toast.success('校场音源缓存已清理')
    } catch (error) {
      toast.error('缓存清理失败', { description: String(error) })
    }
  }

  return (
    <div className="grid gap-4">
      <SettingsSurface className="p-4">
        <div className="mb-4 flex items-start justify-between gap-3">
          <div>
            <div className="flex items-center gap-2 text-[13px] font-semibold">
              <ShieldCheck className="h-4 w-4 text-jade" />
              插件音源 Manifest
            </div>
            <p className="mt-1 max-w-2xl text-[12px] leading-5 text-muted-foreground">
              只注册受限 manifest：renderer 不执行插件代码，播放前由 Rust worker 解析 URL，并校验 allowed_hosts。
            </p>
          </div>
          <Button type="button" size="sm" variant="outline" onClick={() => void refresh()} disabled={loading}>
            <RefreshCw className={loading ? 'animate-spin' : ''} />
            刷新
          </Button>
        </div>

        <div className="grid gap-3 lg:grid-cols-2">
          <CompactInput
            label="plugin_id"
            placeholder="demo.plugin"
            value={draft.pluginId}
            onChange={(event) => updateDraft('pluginId', event.currentTarget.value)}
          />
          <CompactInput
            label="name"
            placeholder="Demo Music Source"
            value={draft.name}
            onChange={(event) => updateDraft('name', event.currentTarget.value)}
          />
          <CompactInput
            label="version"
            placeholder="0.1.0"
            value={draft.version}
            onChange={(event) => updateDraft('version', event.currentTarget.value)}
          />
          <label className="flex items-end gap-2 rounded-xl border border-border/70 bg-muted/20 px-3 py-2 text-[12px]">
            <input
              type="checkbox"
              checked={draft.enabled}
              onChange={(event) => updateDraft('enabled', event.currentTarget.checked)}
              className="mb-1 accent-jade"
            />
            <span>
              <span className="block text-[10.5px] font-semibold uppercase tracking-widest text-muted-foreground/70">
                enabled
              </span>
              启用该插件音源
            </span>
          </label>
        </div>

        <div className="mt-3 grid gap-3">
          <TextAreaField
            label="allowed_hosts"
            placeholder="music.example.test, cdn.example.test"
            value={draft.allowedHosts}
            onChange={(value) => updateDraft('allowedHosts', value)}
          />
          <TextAreaField
            label="resolver_template"
            placeholder="https://music.example.test/{source}/{source_track_id}?quality={quality}"
            value={draft.resolverTemplate}
            onChange={(value) => updateDraft('resolverTemplate', value)}
          />
        </div>

        <div className="mt-4 flex flex-wrap items-center justify-between gap-3">
          <div className="flex items-center gap-2 text-[12px] text-muted-foreground">
            {validationError ? (
              <>
                <AlertCircle className="h-4 w-4 text-amber-600" />
                {validationError}
              </>
            ) : (
              <>
                <CheckCircle2 className="h-4 w-4 text-jade" />
                Manifest 可注册
              </>
            )}
          </div>
          <Button type="button" size="sm" onClick={() => void handleRegister()} disabled={Boolean(validationError)}>
            <Save />
            注册 / 更新
          </Button>
        </div>
      </SettingsSurface>

      <div className="grid gap-4 xl:grid-cols-[minmax(0,1fr)_320px]">
        <SettingsSurface className="p-4">
          <div className="mb-3 flex items-center justify-between gap-3">
            <div className="flex items-center gap-2 text-[13px] font-semibold">
              <Plug className="h-4 w-4 text-jade" />
              已注册插件音源
            </div>
            <span className="rounded-full bg-muted px-2 py-1 text-[11px] text-muted-foreground">{plugins.length}</span>
          </div>
          <div className="grid gap-2">
            {plugins.length === 0 ? (
              <div className="rounded-xl border border-dashed border-border/80 px-3 py-6 text-center text-[12px] text-muted-foreground">
                暂无插件音源。注册 manifest 后会显示在这里。
              </div>
            ) : plugins.map((plugin) => (
              <button
                key={plugin.plugin_id}
                type="button"
                className="rounded-xl border border-border/70 bg-muted/20 px-3 py-2 text-left transition hover:bg-muted/35"
                onClick={() => setDraft(pluginManifestToDraft(plugin))}
              >
                <div className="flex items-center justify-between gap-2">
                  <div className="truncate text-[13px] font-semibold">{plugin.name}</div>
                  <span className={plugin.enabled ? 'text-[11px] text-jade' : 'text-[11px] text-muted-foreground'}>
                    {plugin.enabled ? 'enabled' : 'disabled'}
                  </span>
                </div>
                <div className="mt-1 truncate font-mono text-[11px] text-muted-foreground">{plugin.plugin_id}</div>
                <div className="mt-2 flex flex-wrap gap-1">
                  {plugin.allowed_hosts.map((host) => (
                    <span key={host} className="rounded-md bg-background px-1.5 py-0.5 font-mono text-[10.5px] text-muted-foreground">
                      {host}
                    </span>
                  ))}
                </div>
              </button>
            ))}
          </div>
        </SettingsSurface>

        <SettingsSurface className="p-4">
          <div className="mb-3 flex items-center gap-2 text-[13px] font-semibold">
            <ListMusic className="h-4 w-4 text-jade" />
            服务端缓存
          </div>
          <div className="rounded-xl border border-border/70 bg-muted/20 p-3">
            <div className="text-[26px] font-semibold tracking-tight">{cacheInfo.entries}</div>
            <div className="text-[11px] text-muted-foreground">resolved URL entries</div>
          </div>
          <Button type="button" className="mt-3 w-full" size="sm" variant="outline" onClick={() => void handleClearCache()}>
            <Eraser />
            清理缓存
          </Button>
        </SettingsSurface>
      </div>

      <SettingsSurface className="p-4">
        <div className="mb-3 flex items-center justify-between gap-3">
          <div className="flex items-center gap-2 text-[13px] font-semibold">
            <ListMusic className="h-4 w-4 text-jade" />
            事件日志
          </div>
          <Button type="button" size="sm" variant="ghost" onClick={() => setEvents([])}>
            清空
          </Button>
        </div>
        <div className="max-h-64 overflow-auto rounded-xl border border-border/70 bg-muted/20">
          {events.length === 0 ? (
            <div className="px-3 py-6 text-center text-[12px] text-muted-foreground">
              暂无事件。注册、解析或 cache 命中后会出现日志。
            </div>
          ) : events.map((event, index) => (
            <div key={`${event.timestamp}-${index}`} className="grid gap-1 border-b border-border/60 px-3 py-2 last:border-b-0">
              <div className="flex flex-wrap items-center gap-2 text-[12px]">
                <span className="rounded-md bg-background px-1.5 py-0.5 font-mono text-[10.5px]">{event.event_type}</span>
                <span className="font-mono text-[11px] text-muted-foreground">{event.plugin_id}</span>
                {event.track_id ? <span className="font-mono text-[11px] text-muted-foreground">{event.track_id}</span> : null}
                <span className="ml-auto text-[10.5px] text-muted-foreground">{formatTime(event.timestamp)}</span>
              </div>
              <div className="text-[12px] text-muted-foreground">{event.message}</div>
              {event.url ? <div className="truncate font-mono text-[10.5px] text-muted-foreground">{event.url}</div> : null}
            </div>
          ))}
        </div>
      </SettingsSurface>
    </div>
  )
}

function TextAreaField({
  label,
  onChange,
  placeholder,
  value,
}: {
  label: string
  onChange: (value: string) => void
  placeholder: string
  value: string
}) {
  return (
    <label className="flex flex-col gap-1.5">
      <span className="text-[10.5px] font-semibold uppercase tracking-widest text-muted-foreground/70">
        {label}
      </span>
      <textarea
        value={value}
        placeholder={placeholder}
        onChange={(event) => onChange(event.currentTarget.value)}
        rows={3}
        className="min-h-[76px] rounded-xl border border-border/70 bg-muted/30 px-3 py-2 font-mono text-[12px] outline-none transition-all placeholder:text-muted-foreground/40 focus:border-jade/40 focus:ring-[3px] focus:ring-jade/15"
      />
    </label>
  )
}

function formatTime(value: string): string {
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return value
  return date.toLocaleTimeString()
}
