import { useEffect, useMemo, useState } from 'react'
import {
  AlertTriangle,
  CheckCircle2,
  Copy,
  ExternalLink,
  Globe,
  Loader2,
  Rocket,
  Shield,
  Sparkles,
  RotateCcw,
} from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Switch } from '@/components/ui/switch'
import { AgentOrb } from '@/components/AgentOrb'
import { SettingsSurface } from '../components/SettingsSurface'
import type { SettingsPageProps } from '../types'
import { configResetOnboarding } from '@/lib/tauri'
import { toast } from 'sonner'
import { broadcastChange } from '@/lib/crossWindowSync'
import { APP_VERSION_LABEL } from '@/lib/appVersion'
import {
  checkAppUpdater,
  downloadAndInstallAppUpdate,
  downloadAndOpenAppUpdate,
  getAppUpdaterState,
  onAppUpdaterState,
  setAppUpdaterPreferences,
  type UpdaterCheckResult,
  type UpdaterRuntimeState,
} from '@/api/updater'
import {
  APP_UPDATER_STATE_LABELS,
  updaterProgressPercent,
  updaterUiStateFromResult,
  updaterUiStateFromRuntime,
  type AppUpdaterUiState,
} from './app-updater-state'

const btnOutline =
  'window-no-drag h-7 rounded-xl border border-black/[0.09] bg-black/[0.025] px-3 text-[11.5px] font-medium shadow-none hover:bg-black/[0.05]'

const btnPrimary =
  'window-no-drag h-7 rounded-xl bg-jade px-3 text-[11.5px] font-medium text-white shadow-none hover:bg-jade/90'

const channelLabels = {
  stable: 'Stable 通道',
  beta: 'Beta 通道',
  nightly: 'Nightly 通道',
} as const

function SectionLabel({ children }: { children: React.ReactNode }) {
  return (
    <div className="mb-3 text-[10.5px] font-semibold uppercase tracking-widest text-black/30">
      {children}
    </div>
  )
}

interface FeatureCardProps {
  icon: typeof Shield
  title: string
  text: string
  accent: string
}

function FeatureCard({ icon: Icon, title, text, accent }: FeatureCardProps) {
  return (
    <div className="flex flex-col gap-2 rounded-xl border border-black/[0.06] bg-black/[0.016] px-4 py-3">
      <div
        className="flex size-8 items-center justify-center rounded-xl"
        style={{ background: `${accent}14` }}
      >
        <Icon className="h-4 w-4" style={{ color: accent }} />
      </div>
      <div>
        <div className="text-[12.5px] font-semibold tracking-tight">{title}</div>
        <p className="mt-0.5 text-[11px] leading-4 text-muted-foreground">{text}</p>
      </div>
    </div>
  )
}

export function AboutSettingsPage({}: SettingsPageProps) {
  const [updaterState, setUpdaterState] = useState<UpdaterRuntimeState | null>(null)
  const [checkResult, setCheckResult] = useState<UpdaterCheckResult | null>(null)
  const [uiState, setUiState] = useState<AppUpdaterUiState>('idle')

  useEffect(() => {
    let cancelled = false
    void getAppUpdaterState()
      .then((state) => {
        if (!cancelled) {
          setUpdaterState(state)
          setUiState(updaterUiStateFromRuntime(state))
        }
      })
      .catch((error) => {
        if (!cancelled) {
          setUiState('failed')
          setCheckResult({
            status: 'failed',
            current_version: APP_VERSION_LABEL.replace(/^v/, ''),
            diagnostic: String(error),
          })
        }
      })
    return () => {
      cancelled = true
    }
  }, [])

  useEffect(() => {
    let unlisten: (() => void) | null = null
    let cancelled = false
    void onAppUpdaterState((state) => {
      setUpdaterState(state)
      setUiState(updaterUiStateFromRuntime(state))
    }).then((dispose) => {
      if (cancelled) dispose()
      else unlisten = dispose
    })
    return () => {
      cancelled = true
      unlisten?.()
    }
  }, [])

  const updaterCopy = APP_UPDATER_STATE_LABELS[uiState]
  const effectiveLatestVersion = checkResult?.latest_version ?? updaterState?.latest_version
  const effectiveReleaseNotesUrl = checkResult?.release_notes_url ?? updaterState?.release_notes_url
  const effectiveArtifactUrl = checkResult?.artifact_url ?? updaterState?.artifact_url
  const updaterProgress = updaterProgressPercent(
    updaterState?.downloaded_bytes,
    updaterState?.total_bytes,
  )
  const updaterDetail = useMemo(() => {
    const current = updaterState?.current_version ?? checkResult?.current_version ?? APP_VERSION_LABEL.replace(/^v/, '')
    const channel = updaterState?.channel ? channelLabels[updaterState.channel] : 'Stable 通道'
    if (effectiveLatestVersion) {
      return `当前 v${current.replace(/^v/, '')} · 最新 v${effectiveLatestVersion.replace(/^v/, '')} · ${channel}`
    }
    if (updaterState?.checked_at) return `当前 v${current.replace(/^v/, '')} · ${channel} · 已检查`
    return `当前 ${APP_VERSION_LABEL} · ${channel} · 使用默认更新源`
  }, [checkResult, effectiveLatestVersion, updaterState])

  const handleCheckUpdate = async () => {
    setUiState('checking')
    setCheckResult(null)
    try {
      const result = await checkAppUpdater()
      setCheckResult(result)
      setUiState(updaterUiStateFromResult(result))
      if (result.status === 'update_available') {
        toast.success('发现新版本', {
          description: `If2Ai ${result.latest_version ?? ''} 可用`,
        })
      } else if (result.status === 'no_update') {
        toast.success('已是最新版本')
      } else {
        toast.error('检查更新失败', {
          description: result.diagnostic ?? APP_UPDATER_STATE_LABELS.failed.description,
        })
      }
    } catch (error) {
      setUiState('failed')
      setCheckResult({
        status: 'failed',
        current_version: updaterState?.current_version ?? APP_VERSION_LABEL.replace(/^v/, ''),
        diagnostic: String(error),
      })
      toast.error('检查更新失败', { description: String(error) })
    }
  }

  const openArtifact = () => {
    const url = effectiveReleaseNotesUrl ?? effectiveArtifactUrl
    if (!url) return
    window.open(url, '_blank', 'noopener,noreferrer')
  }

  const handleDownloadUpdate = async () => {
    setUiState('downloading')
    try {
      const result = await downloadAndInstallAppUpdate()
      if (result.status === 'installing' || result.status === 'downloaded') {
        toast.success('更新安装已启动', {
          description: '系统安装器已接管流程，If2Ai 可能会自动退出或重启。',
        })
        setUiState(result.status === 'installing' ? 'installing' : 'downloaded')
      } else if (result.status === 'no_update') {
        toast.success('已是最新版本')
        setUiState('ready')
      } else {
        toast.error('下载更新失败', { description: result.diagnostic ?? '请稍后重试。' })
        setUiState('failed')
      }
    } catch (error) {
      setUiState('failed')
      toast.error('下载更新失败', { description: String(error) })
    }
  }

  const handleLegacyDownload = async () => {
    setUiState('downloading')
    try {
      const result = await downloadAndOpenAppUpdate(updaterState?.manifest_url ?? null)
      if (result.status === 'downloaded') {
        toast.success('更新包已下载', {
          description: result.local_path
            ? `已打开安装包：${result.local_path}`
            : '已打开系统安装器，请按提示完成安装。',
        })
        setUiState('downloaded')
      } else if (result.status === 'no_update') {
        toast.success('已是最新版本')
        setUiState('ready')
      } else {
        toast.error('下载更新失败', { description: result.diagnostic ?? '请稍后重试。' })
        setUiState('failed')
      }
    } catch (error) {
      setUiState('failed')
      toast.error('下载更新失败', { description: String(error) })
    }
  }

  const handleAutoCheckChange = async (checked: boolean) => {
    const next = {
      auto_check_enabled: checked,
      channel: updaterState?.channel ?? 'stable',
    } as const
    setUpdaterState((state) => (state ? { ...state, auto_check_enabled: checked } : state))
    try {
      const state = await setAppUpdaterPreferences(next)
      setUpdaterState(state)
      setUiState(updaterUiStateFromRuntime(state))
    } catch (error) {
      toast.error('保存更新偏好失败', { description: String(error) })
    }
  }

  const copyDiagnostic = async () => {
    const detail = updaterState?.diagnostic ?? checkResult?.diagnostic
    if (!detail) return
    await navigator.clipboard.writeText(detail)
    toast.success('诊断信息已复制')
  }

  return (
    <div className="flex flex-col gap-3">
      {/* ── Hero ── */}
      <SettingsSurface className="px-6 py-6">
        <div className="flex flex-col items-center gap-5 sm:flex-row sm:items-center sm:justify-between">
          <div className="flex items-center gap-4">
            <AgentOrb status="idle" size="hero" />
            <div className="space-y-0.5">
              <div className="text-[19px] font-bold tracking-tight">If2Ai</div>
              <div className="text-[11.5px] text-muted-foreground">桌面 AI 智能体工作台</div>
              <div className="font-mono text-[10px] text-black/25">{APP_VERSION_LABEL}</div>
            </div>
          </div>

          <div className="flex flex-col gap-2.5 sm:items-end">
            <p className="max-w-[280px] text-[12px] leading-5 text-muted-foreground sm:text-right">
              基于 Tauri + Rust + React 的桌面智能体工作台，强调项目分组、会话流式输出、可观测性和本地化执行体验。
            </p>
            <div className="flex gap-2">
              <Button variant="outline" className={btnOutline}>
                <ExternalLink className="mr-1.5 h-3 w-3" />
                查看文档
              </Button>
            </div>
          </div>
        </div>
      </SettingsSurface>

      {/* ── App updater ── */}
      <SettingsSurface className="px-5 py-4">
        <div className="flex flex-col gap-4">
          <div className="min-w-0">
            <div className="mb-3 flex items-center justify-between gap-3">
              <SectionLabel>软件更新</SectionLabel>
              <div className="flex items-center gap-2 rounded-full border border-black/[0.07] bg-black/[0.02] px-2.5 py-1 text-[10.5px] text-muted-foreground">
                <span>自动检查</span>
                <Switch
                  checked={updaterState?.auto_check_enabled ?? true}
                  onCheckedChange={(checked) => void handleAutoCheckChange(checked)}
                  aria-label="自动检查更新"
                />
              </div>
            </div>
            <div className="flex items-start gap-3">
              <span className="mt-0.5 flex size-9 items-center justify-center rounded-xl bg-jade/10 text-jade">
                {uiState === 'failed' ? <AlertTriangle className="h-4 w-4 text-amber-700" /> : null}
                {uiState === 'ready' ? <CheckCircle2 className="h-4 w-4" /> : null}
                {uiState === 'checking' || uiState === 'downloading' || uiState === 'installing' ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : null}
                {uiState === 'idle' || uiState === 'available' || uiState === 'downloaded' ? (
                  <Rocket className="h-4 w-4" />
                ) : null}
              </span>
              <div className="min-w-0 flex-1">
                <div className="text-[13.5px] font-semibold tracking-tight">
                  {uiState === 'available' && effectiveLatestVersion
                    ? `发现新版本 v${effectiveLatestVersion.replace(/^v/, '')}`
                    : updaterCopy.title}
                </div>
                <div className="mt-0.5 text-[11.5px] leading-5 text-muted-foreground">
                  {updaterDetail}
                </div>
                <p className="mt-1.5 max-w-2xl text-[11px] leading-5 text-muted-foreground">
                  {updaterCopy.description}
                </p>
              </div>
            </div>
          </div>

          {(uiState === 'downloading' || uiState === 'installing' || uiState === 'downloaded') && (
            <div className="rounded-xl border border-black/[0.06] bg-black/[0.018] px-3 py-2.5">
              <div className="mb-2 flex items-center justify-between text-[10.5px] text-muted-foreground">
                <span>
                  {uiState === 'installing'
                    ? '正在交给系统安装器'
                    : uiState === 'downloaded'
                      ? '下载完成'
                      : '正在下载更新包'}
                </span>
                <span>{updaterProgress === null ? '校验中' : `${updaterProgress}%`}</span>
              </div>
              <div className="h-1.5 overflow-hidden rounded-full bg-black/[0.06]">
                <div
                  className={`h-full rounded-full bg-jade transition-all ${
                    updaterProgress === null ? 'w-1/3 animate-pulse' : ''
                  }`}
                  style={updaterProgress === null ? undefined : { width: `${updaterProgress}%` }}
                />
              </div>
            </div>
          )}

          {uiState === 'failed' && (updaterState?.diagnostic || checkResult?.diagnostic) ? (
            <div className="rounded-xl border border-amber-500/20 bg-amber-500/[0.06] px-3 py-2.5">
              <div className="text-[11.5px] font-medium text-amber-800">检查更新失败</div>
              <div className="mt-1 line-clamp-2 text-[11px] leading-5 text-amber-900/70">
                {updaterState?.diagnostic ?? checkResult?.diagnostic}
              </div>
              <Button variant="outline" className={`${btnOutline} mt-2`} onClick={copyDiagnostic}>
                <Copy className="mr-1.5 h-3 w-3" />
                复制诊断信息
              </Button>
            </div>
          ) : null}

          <div className="flex flex-wrap justify-end gap-2">
            <Button
              variant="outline"
              className={btnOutline}
              disabled={!effectiveReleaseNotesUrl && !effectiveArtifactUrl}
              onClick={openArtifact}
            >
              <ExternalLink className="mr-1.5 h-3 w-3" />
              查看发布说明
            </Button>
            <Button
              variant="outline"
              className={btnOutline}
              disabled={uiState !== 'failed'}
              onClick={() => void handleLegacyDownload()}
            >
              <ExternalLink className="mr-1.5 h-3 w-3" />
              兼容下载
            </Button>
            {uiState === 'available' ? (
              <Button className={btnPrimary} onClick={() => void handleDownloadUpdate()}>
                <ExternalLink className="mr-1.5 h-3 w-3" />
                下载并安装
              </Button>
            ) : null}
            <Button
              className={uiState === 'available' ? btnOutline : btnPrimary}
              variant={uiState === 'available' ? 'outline' : 'default'}
              onClick={() => void handleCheckUpdate()}
              disabled={uiState === 'checking' || uiState === 'downloading' || uiState === 'installing'}
            >
              {uiState === 'downloading' ? (
                <Loader2 className="mr-1.5 h-3 w-3 animate-spin" />
              ) : uiState === 'checking' ? (
                <Loader2 className="mr-1.5 h-3 w-3 animate-spin" />
              ) : (
                <Rocket className="mr-1.5 h-3 w-3" />
              )}
              {uiState === 'checking' ? '正在检查' : '检查更新'}
            </Button>
          </div>
        </div>
      </SettingsSurface>

      {/* ── Design principles ── */}
      <SettingsSurface className="px-5 py-4">
        <SectionLabel>设计原则</SectionLabel>
        <div className="grid gap-2 sm:grid-cols-3">
          <FeatureCard
            icon={Sparkles}
            title="Agent 优先"
            text="层次克制、信息密度清晰"
            accent="var(--jade)"
          />
          <FeatureCard
            icon={Globe}
            title="跨平台"
            text="macOS / Windows / Linux 统一桌面壳"
            accent="#3b82f6"
          />
          <FeatureCard
            icon={Shield}
            title="可扩展"
            text="未来接入插件、工具面板与评估系统"
            accent="#8b5cf6"
          />
        </div>
      </SettingsSurface>

      {/* ── Danger zone ── */}
      <SettingsSurface className="px-5 py-3.5">
        <div className="flex items-center justify-between">
          <div>
            <div className="text-[12.5px] font-medium">重置 Onboarding</div>
            <div className="mt-0.5 text-[11px] text-muted-foreground">
              清除配置并重新进入引导流程
            </div>
          </div>
          <Button
            variant="outline"
            className={btnOutline}
            onClick={async () => {
              if (!window.confirm('确定要重置 Onboarding 吗？所有配置将被清除。')) return
              try {
                await configResetOnboarding()
                // 跨窗口通知主窗口：立即重新加载 app state，进入 Onboarding 而不需要重启
                void broadcastChange('cross:onboarding-reset', {})
                toast.success('Onboarding 已重置', { description: '主窗口将立即重新进入引导流程' })
              } catch (error) {
                toast.error('重置失败', { description: String(error) })
              }
            }}
          >
            <RotateCcw className="mr-1.5 h-3 w-3" />
            重置
          </Button>
        </div>
      </SettingsSurface>
    </div>
  )
}
