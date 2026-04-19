/**
 * STT 配置页 —— 用户选择 STT provider 并管理对应配置。
 *
 * 两条路径：
 * 1. **Whisper（本地）**：whisper.cpp + Metal 加速，需下载 ggml-*.bin 模型（~150MB）
 *    - 支持一键下载 ggml-base.bin / ggml-small.bin
 *    - 完全离线、隐私安全
 * 2. **Groq Whisper API（云端）**：whisper-large-v3-turbo 极快推理，零本地存储
 *    - 需 Groq API Key（gsk_...）从 https://console.groq.com/keys 获取
 *    - 联网，音频上传到 Groq 服务器
 *
 * 用户切换 provider 后立刻生效，输入框的麦克按钮自动用新 provider。
 */

import { useCallback, useEffect, useRef, useState } from 'react'
import { toast } from 'sonner'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import {
  CheckCircle, AlertCircle, Loader2, Download, Cloud, Cpu,
  ExternalLink, Eye, EyeOff, Sparkles,
} from 'lucide-react'
import { SettingsSurface } from '../components/SettingsSurface'
import {
  sttModelStatus,
  sttDownloadWhisperModel,
  sttDownloadOpenflowModel,
  sttGetSettings,
  sttSaveSettings,
  type SttSettingsDto,
  type SttModelStatusResponse,
  type OpenFlowDownloadProgress,
} from '@/lib/tauri'
import { broadcastChange } from '@/lib/crossWindowSync'

function SectionLabel({ children }: { children: React.ReactNode }) {
  return (
    <div className="mb-3 text-[10.5px] font-semibold uppercase tracking-widest text-black/30">
      {children}
    </div>
  )
}

const WHISPER_MODELS = [
  { id: 'ggml-base.bin', label: 'Base (148MB)', desc: '推荐 · 平衡精度与速度，多语言' },
  { id: 'ggml-small.bin', label: 'Small (~466MB)', desc: '更高精度，多语言；M-series 仍流畅' },
  { id: 'ggml-tiny.bin', label: 'Tiny (~75MB)', desc: '最小最快，精度较低' },
]

export function SttConfigPage() {
  const [settings, setSettings] = useState<SttSettingsDto | null>(null)
  const [whisperStatus, setWhisperStatus] = useState<SttModelStatusResponse | null>(null)
  const [downloadingModelId, setDownloadingModelId] = useState<string | null>(null)
  const [groqApiKey, setGroqApiKey] = useState('')
  const [showGroqKey, setShowGroqKey] = useState(false)
  const [savingProvider, setSavingProvider] = useState(false)
  // SenseVoice 下载状态
  const [openflowDownloading, setOpenflowDownloading] = useState(false)
  const [openflowProgress, setOpenflowProgress] = useState<OpenFlowDownloadProgress | null>(null)
  const unlistenRef = useRef<UnlistenFn | null>(null)

  const refresh = useCallback(async () => {
    try {
      const [s, w] = await Promise.all([sttGetSettings(), sttModelStatus()])
      setSettings(s)
      setWhisperStatus(w)
    } catch (e) {
      console.error('refresh failed', e)
    }
  }, [])

  useEffect(() => { void refresh() }, [refresh])

  // 订阅 SenseVoice 下载进度事件
  useEffect(() => {
    let mounted = true
    void listen<OpenFlowDownloadProgress>('stt:openflow-download-progress', (e) => {
      if (mounted) setOpenflowProgress(e.payload)
    }).then((un) => {
      if (mounted) unlistenRef.current = un
      else un()
    })
    return () => {
      mounted = false
      unlistenRef.current?.()
      unlistenRef.current = null
    }
  }, [])

  const handleDownloadOpenflow = useCallback(async () => {
    if (openflowDownloading) return
    setOpenflowDownloading(true)
    setOpenflowProgress(null)
    toast.info('开始下载 SenseVoice 模型', {
      description: '约 230MB，根据网速 1-5 分钟。下载源会自动 fallback hf-mirror。',
    })
    try {
      const path = await sttDownloadOpenflowModel({ preset: 'quantized' })
      toast.success('SenseVoice 下载完成', {
        description: path.split('/').slice(-2).join('/'),
        duration: 4000,
      })
      // 跨窗口通知主窗口 SttButton：openflow 模型从未就绪 → 已就绪
      void broadcastChange('cross:stt-settings-changed', { openflow_ready: true })
      await refresh()
    } catch (e) {
      toast.error('下载失败', { description: String(e) })
    } finally {
      setOpenflowDownloading(false)
      setOpenflowProgress(null)
    }
  }, [openflowDownloading, refresh])

  const handleSwitchProvider = useCallback(async (provider: 'whisper' | 'groq' | 'openflow') => {
    setSavingProvider(true)
    try {
      const updated = await sttSaveSettings({ provider })
      setSettings(updated)
      // 跨窗口广播：主窗口的 SttButton 立即重新拉 settings
      void broadcastChange('cross:stt-settings-changed', { provider })
      const label = provider === 'whisper' ? 'Whisper 本地'
        : provider === 'groq' ? 'Groq 云端'
        : 'SenseVoice 本地'
      toast.success(`已切换到 ${label}`)
    } catch (e) {
      toast.error('切换失败', { description: String(e) })
    } finally {
      setSavingProvider(false)
    }
  }, [])

  const handleSaveGroqKey = useCallback(async () => {
    if (!groqApiKey.trim()) {
      toast.error('请填写 API Key')
      return
    }
    try {
      const updated = await sttSaveSettings({ groq_api_key: groqApiKey.trim() })
      setSettings(updated)
      setGroqApiKey('')
      void broadcastChange('cross:stt-settings-changed', { groq_api_key_set: true })
      toast.success('Groq API Key 已保存')
    } catch (e) {
      toast.error('保存失败', { description: String(e) })
    }
  }, [groqApiKey])

  const handleDownloadModel = useCallback(async (modelId: string) => {
    setDownloadingModelId(modelId)
    toast.info(`开始下载 ${modelId}`, {
      description: '从 HuggingFace 拉取中，过程不可中断；首次约 30-90 秒',
    })
    try {
      const path = await sttDownloadWhisperModel(modelId)
      toast.success('下载完成', { description: `已保存到 ${path.split('/').slice(-2).join('/')}` })
      await refresh()
    } catch (e) {
      toast.error('下载失败', { description: String(e) })
    } finally {
      setDownloadingModelId(null)
    }
  }, [refresh])

  const isProviderActive = (p: string) => settings?.provider === p

  return (
    <div className="flex flex-col gap-3">
      <SettingsSurface className="px-5 py-4">
        <SectionLabel>STT 语音输入 — 后端选择</SectionLabel>
        <p className="mb-3 text-[11px] text-muted-foreground">
          选择语音转文字的引擎。可随时切换，配置保留。
        </p>
        <div className="grid grid-cols-1 gap-2 sm:grid-cols-3">
          {/* SenseVoice (open-flow vendor) */}
          <button
            type="button"
            disabled={savingProvider}
            onClick={() => void handleSwitchProvider('openflow')}
            className={`flex flex-col gap-1.5 rounded-xl border p-3 text-left transition-colors disabled:opacity-50 ${
              isProviderActive('openflow')
                ? 'border-jade/50 bg-jade/8'
                : 'border-black/[0.08] bg-white hover:bg-black/[0.018]'
            }`}
          >
            <div className="flex items-center gap-2">
              <Sparkles className={`size-4 ${isProviderActive('openflow') ? 'text-jade' : 'text-black/40'}`} />
              <span className="flex-1 text-[12px] font-semibold">SenseVoice 本地（open-flow）</span>
              {isProviderActive('openflow') && (
                <CheckCircle className="size-3.5 text-jade" />
              )}
            </div>
            <p className="text-[10.5px] text-muted-foreground">中文识别强 · ONNX 推理 · 离线 · 230MB 量化模型</p>
            <div className="text-[10px]">
              {whisperStatus?.openflow_ready ? (
                <span className="text-jade">✓ 模型已就绪</span>
              ) : (
                <span className="text-amber-600">⚠ 模型未下载</span>
              )}
            </div>
          </button>

          {/* Whisper local */}
          <button
            type="button"
            disabled={savingProvider}
            onClick={() => void handleSwitchProvider('whisper')}
            className={`flex flex-col gap-1.5 rounded-xl border p-3 text-left transition-colors disabled:opacity-50 ${
              isProviderActive('whisper')
                ? 'border-jade/50 bg-jade/8'
                : 'border-black/[0.08] bg-white hover:bg-black/[0.018]'
            }`}
          >
            <div className="flex items-center gap-2">
              <Cpu className={`size-4 ${isProviderActive('whisper') ? 'text-jade' : 'text-black/40'}`} />
              <span className="flex-1 text-[12px] font-semibold">Whisper 本地（whisper.cpp）</span>
              {isProviderActive('whisper') && (
                <CheckCircle className="size-3.5 text-jade" />
              )}
            </div>
            <p className="text-[10.5px] text-muted-foreground">Metal 加速 · 完全离线 · 需下载 ~150MB 模型</p>
            <div className="text-[10px]">
              {whisperStatus?.ready ? (
                <span className="text-jade">✓ 已就绪 · {whisperStatus.model_name}</span>
              ) : (
                <span className="text-amber-600">⚠ 模型未下载</span>
              )}
            </div>
          </button>

          {/* Groq cloud */}
          <button
            type="button"
            disabled={savingProvider}
            onClick={() => void handleSwitchProvider('groq')}
            className={`flex flex-col gap-1.5 rounded-xl border p-3 text-left transition-colors disabled:opacity-50 ${
              isProviderActive('groq')
                ? 'border-jade/50 bg-jade/8'
                : 'border-black/[0.08] bg-white hover:bg-black/[0.018]'
            }`}
          >
            <div className="flex items-center gap-2">
              <Cloud className={`size-4 ${isProviderActive('groq') ? 'text-jade' : 'text-black/40'}`} />
              <span className="flex-1 text-[12px] font-semibold">Groq Whisper API（云端）</span>
              {isProviderActive('groq') && (
                <CheckCircle className="size-3.5 text-jade" />
              )}
            </div>
            <p className="text-[10.5px] text-muted-foreground">whisper-large-v3-turbo · 极速 · 零本地模型</p>
            <div className="text-[10px]">
              {settings?.groq_api_key_set ? (
                <span className="text-jade">✓ API Key 已配置</span>
              ) : (
                <span className="text-amber-600">⚠ 需配置 API Key</span>
              )}
            </div>
          </button>
        </div>
      </SettingsSurface>

      {/* ── Whisper 模型管理 ── */}
      <SettingsSurface className="px-5 py-4">
        <div className="mb-2 flex items-center justify-between">
          <SectionLabel>Whisper 本地模型</SectionLabel>
          {whisperStatus?.model_dir && (
            <span className="text-[9.5px] font-mono text-black/30">
              {whisperStatus.model_dir.replace(/.*\/Users\/[^/]+/, '~')}
            </span>
          )}
        </div>
        <div className="flex flex-col gap-2">
          {WHISPER_MODELS.map((m) => {
            const installed = whisperStatus?.ready && whisperStatus.model_name === m.id
            const isDownloading = downloadingModelId === m.id
            return (
              <div
                key={m.id}
                className={`flex items-center gap-3 rounded-xl border px-3 py-2 ${
                  installed
                    ? 'border-jade/40 bg-jade/5'
                    : 'border-black/[0.07] bg-white'
                }`}
              >
                <div className="flex-1 min-w-0">
                  <div className="text-[11.5px] font-semibold">{m.label}</div>
                  <div className="text-[10px] text-muted-foreground">{m.desc}</div>
                </div>
                {installed ? (
                  <span className="flex items-center gap-1 text-[10px] text-jade">
                    <CheckCircle className="size-3" /> 已安装
                  </span>
                ) : (
                  <button
                    type="button"
                    onClick={() => void handleDownloadModel(m.id)}
                    disabled={isDownloading || !!downloadingModelId}
                    className="flex items-center gap-1 rounded-lg bg-foreground/85 px-2.5 py-1 text-[10.5px] font-semibold text-white hover:bg-foreground disabled:opacity-50"
                  >
                    {isDownloading ? (
                      <>
                        <Loader2 className="size-3 animate-spin" />
                        下载中…
                      </>
                    ) : (
                      <>
                        <Download className="size-3" />
                        下载
                      </>
                    )}
                  </button>
                )}
              </div>
            )
          })}
        </div>
        <p className="mt-2 text-[10px] text-muted-foreground">
          模型来自 HuggingFace ggerganov/whisper.cpp。下载后写入 {whisperStatus?.model_dir ?? '~/.if2ai/models/whisper/'}。
        </p>
      </SettingsSurface>

      {/* ── SenseVoice (OpenFlow) 模型 ── */}
      <SettingsSurface className="px-5 py-4">
        <div className="mb-2 flex items-center justify-between">
          <SectionLabel>SenseVoice 模型（中文优势 · open-flow）</SectionLabel>
          {whisperStatus?.openflow_model_dir && (
            <span className="text-[9.5px] font-mono text-black/30">
              {whisperStatus.openflow_model_dir.replace(/.*\/Users\/[^/]+/, '~')}
            </span>
          )}
        </div>
        <div
          className={`flex items-center gap-3 rounded-xl border px-3 py-2 ${
            whisperStatus?.openflow_ready
              ? 'border-jade/40 bg-jade/5'
              : 'border-black/[0.07] bg-white'
          }`}
        >
          <div className="flex-1 min-w-0">
            <div className="text-[11.5px] font-semibold">
              SenseVoice-Small Quantized
              <span className="ml-1.5 text-[9.5px] font-normal text-muted-foreground">~230 MB</span>
            </div>
            <div className="text-[10px] text-muted-foreground">
              FunASR 出品 · 中/英/粤/日/韩 · ONNX CPU 推理 · 自带标点
            </div>
            {openflowDownloading && openflowProgress && (
              <div className="mt-1 flex items-center gap-2 text-[10px]">
                <div className="h-1.5 flex-1 overflow-hidden rounded-full bg-black/[0.08]">
                  <div
                    className="h-full rounded-full bg-jade transition-all"
                    style={{
                      width: openflowProgress.percent >= 0 ? `${openflowProgress.percent}%` : '40%',
                    }}
                  />
                </div>
                <span className="font-mono text-black/55">
                  {openflowProgress.file} ·{' '}
                  {(openflowProgress.downloaded / 1_048_576).toFixed(1)} MB
                  {openflowProgress.percent >= 0 ? ` (${openflowProgress.percent}%)` : ''}
                </span>
              </div>
            )}
          </div>
          {whisperStatus?.openflow_ready ? (
            <span className="flex items-center gap-1 text-[10px] text-jade">
              <CheckCircle className="size-3" /> 已安装
            </span>
          ) : (
            <button
              type="button"
              onClick={() => void handleDownloadOpenflow()}
              disabled={openflowDownloading}
              className="flex items-center gap-1 rounded-lg bg-foreground/85 px-2.5 py-1 text-[10.5px] font-semibold text-white hover:bg-foreground disabled:opacity-50"
            >
              {openflowDownloading ? (
                <>
                  <Loader2 className="size-3 animate-spin" />
                  下载中…
                </>
              ) : (
                <>
                  <Download className="size-3" />
                  一键下载（230MB）
                </>
              )}
            </button>
          )}
        </div>
        <p className="mt-2 text-[10px] text-muted-foreground">
          来源：HuggingFace haixuantao/SenseVoiceSmall-onnx；自动 fallback 到 hf-mirror.com 国内镜像。
        </p>
      </SettingsSurface>

      {/* ── Groq API Key ── */}
      <SettingsSurface className="px-5 py-4">
        <div className="mb-2 flex items-center justify-between">
          <SectionLabel>Groq API Key</SectionLabel>
          <a
            href="https://console.groq.com/keys"
            target="_blank"
            rel="noreferrer"
            className="flex items-center gap-1 text-[10px] text-jade hover:underline"
          >
            <ExternalLink className="size-3" />
            申请 Key
          </a>
        </div>
        <div className="flex items-center gap-2">
          <div className="relative flex-1">
            <input
              type={showGroqKey ? 'text' : 'password'}
              value={groqApiKey}
              onChange={(e) => setGroqApiKey(e.target.value)}
              placeholder={settings?.groq_api_key_set ? '••••••••••（已保存）' : 'gsk_...'}
              className="h-8 w-full rounded-lg border border-black/[0.09] bg-black/[0.025] px-2 pr-8 font-mono text-[11px] outline-none focus:border-jade/40"
            />
            <button
              type="button"
              onClick={() => setShowGroqKey((v) => !v)}
              className="absolute right-1.5 top-1.5 rounded p-1 text-black/40 hover:text-black/70"
            >
              {showGroqKey ? <EyeOff className="size-3.5" /> : <Eye className="size-3.5" />}
            </button>
          </div>
          <button
            type="button"
            onClick={() => void handleSaveGroqKey()}
            disabled={!groqApiKey.trim()}
            className="rounded-lg bg-jade px-3 py-1.5 text-[11px] font-semibold text-white disabled:opacity-50"
          >
            保存
          </button>
        </div>
        <p className="mt-2 text-[10px] text-muted-foreground">
          API Key 加密存于本地 <code className="font-mono">stt_settings.json</code>，不会上传到任何第三方。
          Groq 免费额度足够日常使用，模型默认 <code className="font-mono">{settings?.groq_model ?? 'whisper-large-v3-turbo'}</code>。
        </p>
      </SettingsSurface>

      {/* ── 提示 ── */}
      <SettingsSurface className="px-5 py-3">
        <div className="flex items-start gap-2 text-[10.5px] text-muted-foreground">
          <AlertCircle className="mt-0.5 size-3.5 shrink-0 text-amber-600" />
          <div className="space-y-1">
            <div>
              使用语音输入：聊天输入框右侧的 🎤 麦克按钮 → 点击开始 / 再点结束 → 转写完成自动填入输入框。
            </div>
            <div>
              <strong>SenseVoice / open-flow 等其它后端</strong>：后续 Phase 计划接入；目前可通过 Groq Whisper 立即获得云端高质量识别（无需下载模型）。
            </div>
          </div>
        </div>
      </SettingsSurface>
    </div>
  )
}
