import { useCallback, useEffect, useMemo, useState, type ReactNode } from 'react'
import { AlertCircle, CheckCircle2, Eraser, Fingerprint, ListMusic, Plug, RefreshCw, Save, ScanSearch, ShieldAlert, ShieldCheck, Trash2 } from 'lucide-react'
import { toast } from 'sonner'

import { Button } from '@/components/ui/button'

import {
  clearJiaochangAudioPluginCache,
  getJiaochangAudioPluginCacheInfo,
  importJiaochangAudioLxCeruJsFile,
  inspectJiaochangAudioPluginJsFile,
  installAuthorizedChineseMusicSourceTemplate,
  listJiaochangAudioPluginSources,
  registerJiaochangAudioPluginSource,
  removeJiaochangAudioPluginSource,
  setJiaochangAudioPluginSourceEnabled,
  subscribeJiaochangAudioPluginEvents,
  type JiaochangAudioPluginCacheInfo,
  type JiaochangAudioPluginInspection,
  type JiaochangAudioPluginManifest,
  type JiaochangAudioPluginRuntimeEvent,
} from '@/modules/jiaochang/audio/plugin-source-adapter'
import {
  DEFAULT_JIACHANG_PLUGIN_MANIFEST_DRAFT,
  draftToPluginManifest,
  getPluginTrustAssessment,
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
  const [inspectionPath, setInspectionPath] = useState('/Users/ryanliu/Downloads/music/sixyin-music-source-v1.0.7.js')
  const [inspection, setInspection] = useState<JiaochangAudioPluginInspection | null>(null)
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

  const handleInstallTemplate = async () => {
    try {
      const registered = await installAuthorizedChineseMusicSourceTemplate()
      toast.success('五源授权模板已安装', { description: registered.plugin_id })
      await refresh()
    } catch (error) {
      toast.error('五源授权模板安装失败', { description: String(error) })
    }
  }

  const handleInspectJsPlugin = async () => {
    try {
      const result = await inspectJiaochangAudioPluginJsFile(inspectionPath)
      setInspection(result)
      toast.success('LX/Ceru 插件静态检查完成', { description: result.file_name })
    } catch (error) {
      toast.error('LX/Ceru 插件检查失败', { description: String(error) })
    }
  }

  const handleImportJsPlugin = async () => {
    try {
      const registered = await importJiaochangAudioLxCeruJsFile(inspectionPath)
      toast.success('LX/Ceru 插件已注册到隔离 worker', { description: registered.plugin_id })
      await refresh()
    } catch (error) {
      toast.error('LX/Ceru 插件注册失败', { description: String(error) })
    }
  }

  const togglePluginEnabled = async (plugin: JiaochangAudioPluginManifest) => {
    try {
      const updated = await setJiaochangAudioPluginSourceEnabled(plugin.plugin_id, !plugin.enabled)
      toast.success(updated.enabled ? '插件音源已启用' : '插件音源已停用', { description: updated.plugin_id })
      await refresh()
    } catch (error) {
      toast.error('插件音源状态更新失败', { description: String(error) })
    }
  }

  const removePlugin = async (plugin: JiaochangAudioPluginManifest) => {
    if (!window.confirm(`删除音源「${plugin.name}」？此操作会同时清理该音源的服务端缓存。`)) return
    try {
      await removeJiaochangAudioPluginSource(plugin.plugin_id)
      toast.success('插件音源已删除', { description: plugin.plugin_id })
      if (draft.pluginId === plugin.plugin_id) setDraft(DEFAULT_JIACHANG_PLUGIN_MANIFEST_DRAFT)
      await refresh()
    } catch (error) {
      toast.error('插件音源删除失败', { description: String(error) })
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
          <CompactInput
            label="source_url"
            placeholder="https://example.test/plugin/manifest.json"
            value={draft.sourceUrl}
            onChange={(event) => updateDraft('sourceUrl', event.currentTarget.value)}
          />
          <CompactInput
            label="signature"
            placeholder="sha256:..."
            value={draft.signature}
            onChange={(event) => updateDraft('signature', event.currentTarget.value)}
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

        <div className="mt-3 grid gap-2 rounded-xl border border-jade/15 bg-jade/[0.035] p-3 text-[12px] text-muted-foreground md:grid-cols-3">
          <PermissionHint icon={<ShieldCheck className="h-4 w-4" />} title="Renderer 隔离" body="前端只提交 metadata，插件解析在 Rust worker 内完成。" />
          <PermissionHint icon={<ListMusic className="h-4 w-4" />} title="网络权限" body="只允许 resolver_template 解析到 allowed_hosts。" />
          <PermissionHint icon={<Fingerprint className="h-4 w-4" />} title="签名/来源" body="source_url 与 signature 用于人工确认来源可信度。" />
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

      <SettingsSurface className="p-4">
        <div className="mb-4 flex items-start justify-between gap-3">
          <div>
            <div className="flex items-center gap-2 text-[13px] font-semibold">
              <ScanSearch className="h-4 w-4 text-jade" />
              LX / Ceru 兼容导入
            </div>
            <p className="mt-1 max-w-2xl text-[12px] leading-5 text-muted-foreground">
              静态检查后可注册到 Rust-side 隔离 worker：JS 插件不会在 renderer 执行。混淆、动态代码、DOM shim 会被标为风险。
            </p>
          </div>
          <Button type="button" size="sm" variant="outline" onClick={() => void handleInstallTemplate()}>
            <ShieldCheck />
            安装五源授权模板
          </Button>
        </div>
        <div className="grid gap-3 lg:grid-cols-[minmax(0,1fr)_auto]">
          <CompactInput
            label="local_js_plugin_path"
            placeholder="/Users/ryanliu/Downloads/V260418/第三批次/xinghai-music-source2.3.0.js"
            value={inspectionPath}
            onChange={(event) => setInspectionPath(event.currentTarget.value)}
          />
          <Button type="button" size="sm" className="self-end" onClick={() => void handleInspectJsPlugin()}>
            <ScanSearch />
            静态检查
          </Button>
        </div>
        <div className="mt-3 flex flex-wrap items-center gap-2">
          <Button type="button" size="sm" variant="outline" onClick={() => setInspectionPath('/Users/ryanliu/Downloads/music/sixyin-music-source-v1.0.7.js')}>
            六音路径
          </Button>
          <Button type="button" size="sm" variant="outline" onClick={() => setInspectionPath('/Users/ryanliu/Downloads/V260418/第三批次/xinghai-music-source2.3.0.js')}>
            星海路径
          </Button>
          <Button type="button" size="sm" onClick={() => void handleImportJsPlugin()}>
            <Plug />
            注册到隔离 worker
          </Button>
        </div>
        {inspection ? (
          <div className="mt-3 rounded-xl border border-border/70 bg-muted/20 p-3 text-[12px]">
            <div className="flex flex-wrap items-center gap-2">
              <span className="font-semibold">{inspection.name ?? inspection.file_name}</span>
              <span className="rounded-md bg-background px-1.5 py-0.5 font-mono text-[10.5px]">{inspection.detected_kind}</span>
              {inspection.version ? <span className="text-muted-foreground">{inspection.version}</span> : null}
            </div>
            <div className="mt-1 truncate font-mono text-[10.5px] text-muted-foreground">sha256:{inspection.sha256}</div>
            <div className="mt-2 text-muted-foreground">{inspection.recommendation}</div>
            <div className="mt-2 flex flex-wrap gap-1">
              {inspection.risk_flags.map((flag) => (
                <span key={flag} className="rounded-md bg-amber-500/10 px-1.5 py-0.5 text-[10.5px] text-amber-700">
                  {flag}
                </span>
              ))}
              {inspection.supported_hosts.map((host) => (
                <span key={host} className="rounded-md bg-background px-1.5 py-0.5 font-mono text-[10.5px] text-muted-foreground">
                  {host}
                </span>
              ))}
            </div>
          </div>
        ) : null}
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
            ) : plugins.map((plugin) => {
              const trust = getPluginTrustAssessment(plugin)
              return (
              <div
                key={plugin.plugin_id}
                className="cursor-pointer rounded-xl border border-border/70 bg-muted/20 px-3 py-2 text-left transition hover:bg-muted/35"
                onClick={() => setDraft(pluginManifestToDraft(plugin))}
                role="button"
                tabIndex={0}
                onKeyDown={(event) => {
                  if (event.key === 'Enter' || event.key === ' ') setDraft(pluginManifestToDraft(plugin))
                }}
              >
                <div className="flex items-center justify-between gap-2">
                  <div className="truncate text-[13px] font-semibold">{plugin.name}</div>
                  <div className="flex items-center gap-2">
                    <span className={trust.level === 'signed' ? 'text-[11px] text-jade' : trust.level === 'scoped' ? 'text-[11px] text-amber-600' : 'text-[11px] text-status-error'}>
                      {trust.label}
                    </span>
                    <span className={plugin.enabled ? 'text-[11px] text-jade' : 'text-[11px] text-muted-foreground'}>
                      {plugin.enabled ? 'enabled' : 'disabled'}
                    </span>
                  </div>
                </div>
                <div className="mt-1 truncate font-mono text-[11px] text-muted-foreground">{plugin.plugin_id}</div>
                <div className="mt-1 flex items-center gap-1 text-[10.5px] text-muted-foreground">
                  {trust.level === 'unverified' ? <ShieldAlert className="h-3 w-3 text-status-error" /> : <ShieldCheck className="h-3 w-3 text-jade" />}
                  <span className="truncate">{trust.description}</span>
                </div>
                <div className="mt-2 flex flex-wrap gap-1">
                  {plugin.allowed_hosts.map((host) => (
                    <span key={host} className="rounded-md bg-background px-1.5 py-0.5 font-mono text-[10.5px] text-muted-foreground">
                      {host}
                    </span>
                  ))}
                </div>
                <div className="mt-2 flex justify-end gap-2">
                  <button
                    type="button"
                    onClick={(event) => {
                      event.stopPropagation()
                      void togglePluginEnabled(plugin)
                    }}
                    onKeyDown={(event) => {
                      if (event.key !== 'Enter' && event.key !== ' ') return
                      event.stopPropagation()
                      void togglePluginEnabled(plugin)
                    }}
                    className="rounded-lg border border-border/70 bg-background px-2 py-1 text-[11px] font-semibold text-foreground/75"
                  >
                    {plugin.enabled ? '停用' : '启用'}
                  </button>
                  <button
                    type="button"
                    onClick={(event) => {
                      event.stopPropagation()
                      void removePlugin(plugin)
                    }}
                    onKeyDown={(event) => {
                      if (event.key !== 'Enter' && event.key !== ' ') return
                      event.stopPropagation()
                      void removePlugin(plugin)
                    }}
                    className="inline-flex items-center gap-1 rounded-lg border border-status-error/25 bg-status-error-bg px-2 py-1 text-[11px] font-semibold text-status-error"
                  >
                    <Trash2 className="h-3 w-3" />
                    删除
                  </button>
                </div>
              </div>
              )
            })}
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

function PermissionHint({ body, icon, title }: { body: string; icon: ReactNode; title: string }) {
  return (
    <div className="flex gap-2">
      <span className="mt-0.5 text-jade">{icon}</span>
      <span>
        <span className="block font-semibold text-foreground/80">{title}</span>
        <span className="leading-5">{body}</span>
      </span>
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
