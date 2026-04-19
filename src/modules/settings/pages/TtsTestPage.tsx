import React, { useCallback, useEffect, useRef, useState } from 'react'
import { toast } from 'sonner'
import { Play, Pause, Loader2, AlertCircle, CheckCircle, RefreshCw } from 'lucide-react'
import { SettingsSurface } from '../components/SettingsSurface'
import {
  ttsHealth,
  ttsWarmupStatus,
  ttsStartWarmup,
  ttsSynthesize,
  ttsStreamStart,
  ttsStreamStatus,
  ttsStreamClose,
  ttsModelStatus,
  ttsSplitText,
  type TtsGenerationParams,
  type TtsProviderState,
  TTS_DEFAULT_PARAMS,
} from '@/lib/tauri'
import { useWebAudioStreamPlayer } from './useWebAudioStreamPlayer'
import { AgentVoicePicker, getAgentVoiceId } from './AgentVoicePicker'

// ── Demo entries (mirrors demo.jsonl from MOSS-TTS-Nano) ─────────────────────

interface DemoEntry {
  id: string
  label: string
  text: string
  role: string
}

const DEMOS: DemoEntry[] = [
  { id: 'demo-0', label: '🇨🇳 欢迎使用', text: '你好，欢迎使用模思智能。', role: 'zh_1' },
  { id: 'demo-1', label: '🇨🇳 科技新闻', text: '人工智能技术正在改变我们的生活方式。', role: 'zh_1' },
  { id: 'demo-2', label: '🇨🇳 诗歌朗诵', text: '床前明月光，疑是地上霜。举头望明月，低头思故乡。', role: 'zh_2' },
  { id: 'demo-3', label: '🇨🇳 日常对话', text: '今天天气真不错，我们一起去公园散步吧。', role: 'zh_3' },
  { id: 'demo-4', label: '🇨🇳 新闻播报', text: '据最新报道，我国成功发射了新一代载人飞船。', role: 'zh_4' },
  { id: 'demo-5', label: '🇨🇳 儿童故事', text: '从前有一只小兔子，它非常喜欢胡萝卜。', role: 'zh_5' },
  { id: 'demo-6', label: '🇨🇳 学术演讲', text: '本文将介绍一种基于深度学习的自然语言处理方法。', role: 'zh_6' },
  { id: 'demo-7', label: '🇨🇳 广告配音', text: '品质生活，从一杯好咖啡开始。', role: 'zh_10' },
  { id: 'demo-8', label: '🇨🇳 语音助手', text: '好的，我已经为你设置了明天早上八点的闹钟。', role: 'zh_11' },
  { id: 'demo-9', label: '🇬🇧 English Welcome', text: 'Hello, welcome to the world of artificial intelligence.', role: 'en_2' },
  { id: 'demo-10', label: '🇬🇧 English News', text: 'The latest breakthrough in AI technology is transforming the industry.', role: 'en_3' },
  { id: 'demo-11', label: '🇬🇧 English Story', text: 'Once upon a time, there was a little robot who dreamed of flying.', role: 'en_4' },
  { id: 'demo-12', label: '🇯🇵 日本語挨拶', text: 'こんにちは、人工智能の世界へようこそ。', role: 'jp_1' },
  { id: 'demo-13', label: '🇯🇵 日本語ニュース', text: '最新のAI技術が、私たちの生活を変えています。', role: 'jp_2' },
  { id: 'demo-14', label: '🇩🇪 Deutsch', text: 'Willkommen in der Welt der künstlichen Intelligenz.', role: 'zh_1' },
  { id: 'demo-15', label: '🇫🇷 Français', text: 'Bienvenue dans le monde de l\'intelligence artificielle.', role: 'zh_2' },
  { id: 'demo-16', label: '🇪🇸 Español', text: 'Bienvenido al mundo de la inteligencia artificial.', role: 'zh_3' },
  { id: 'demo-17', label: '🇰🇷 한국어', text: '인공지능의 세계에 오신 것을 환영합니다.', role: 'zh_4' },
  { id: 'demo-18', label: '🇷🇺 Русский', text: 'Добро пожаловать в мир искусственного интеллекта.', role: 'zh_5' },
  { id: 'demo-19', label: '🇮🇹 Italiano', text: 'Benvenuto nel mondo dell\'intelligenza artificiale.', role: 'zh_6' },
  { id: 'demo-20', label: '🇸🇦 العربية', text: 'مرحبًا بك في عالم الذكاء الاصطناعي.', role: 'zh_10' },
  { id: 'demo-21', label: '🇵🇱 Polski', text: 'Witaj w świecie sztucznej inteligencji.', role: 'zh_11' },
  { id: 'demo-22', label: '🇵🇹 Português', text: 'Bem-vindo ao mundo da inteligência artificial.', role: 'en_5' },
  { id: 'demo-23', label: '🇨🇿 Čeština', text: 'Vítejte ve světě umělé inteligence.', role: 'en_6' },
  { id: 'demo-24', label: '🇩🇰 Dansk', text: 'Velkommen til verden af kunstig intelligens.', role: 'en_7' },
  { id: 'demo-25', label: '🇸🇪 Svenska', text: 'Välkommen till världen av artificiell intelligens.', role: 'en_8' },
  { id: 'demo-26', label: '🇬🇷 Ελληνικά', text: 'Καλώς ήρθατε στον κόσμο της τεχνητής νοημοσύνης.', role: 'jp_3' },
  { id: 'demo-27', label: '🇹🇷 Türkçe', text: 'Yapay zeka dünyasına hoş geldiniz.', role: 'jp_4' },
  { id: 'demo-28', label: '🇭🇺 Magyar', text: 'Üdvözöljük a mesterséges intelligencia világában.', role: 'jp_5' },
]

// ── Helper components ────────────────────────────────────────────────────────

function SectionLabel({ children }: { children: React.ReactNode }) {
  return (
    <div className="mb-3 text-[10.5px] font-semibold uppercase tracking-widest text-black/30">
      {children}
    </div>
  )
}

function StatusBadge({
  state,
  progress,
  message,
}: {
  state: 'ready' | 'initializing' | 'failed' | 'idle'
  progress: number
  message: string
}) {
  const colors = {
    ready: 'bg-emerald-50 text-emerald-700 border-emerald-200/70',
    initializing: 'bg-amber-50 text-amber-700 border-amber-200/70',
    failed: 'bg-rose-50 text-rose-700 border-rose-200/70',
    idle: 'bg-gray-50 text-gray-600 border-gray-200/70',
  }
  const icons = {
    ready: <CheckCircle className="h-3.5 w-3.5" />,
    initializing: <Loader2 className="h-3.5 w-3.5 animate-spin" />,
    failed: <AlertCircle className="h-3.5 w-3.5" />,
    idle: null,
  }
  return (
    <div className={`flex items-center gap-2 rounded-xl border px-3 py-2 text-[11px] ${colors[state]}`}>
      {icons[state]}
      <span className="flex-1 truncate">{message || state}</span>
      {state === 'initializing' && (
        <span className="font-mono text-[10px]">{Math.round(progress * 100)}%</span>
      )}
    </div>
  )
}

// ── Generation Options Panel ─────────────────────────────────────────────────

interface GenerationOptionsPanelProps {
  params: TtsGenerationParams
  onChange: (params: TtsGenerationParams) => void
}

// Phase TTS-C.5：参数分 Basic / Advanced 两组，默认只露 Basic 5 个，
// 减少首次接触用户的认知负担。Advanced 有 13 个 sampling/batch 旋钮。
type ParamSpec = [keyof TtsGenerationParams, string, number, number]
const BASIC_NUMERIC: ParamSpec[] = [
  ['max_new_frames', 'Max New Frames', 64, 1024],
  ['voice_clone_max_text_tokens', 'VC Max Tokens', 25, 200],
  ['audio_temperature', 'Audio Temp', 0.1, 2.0],
]
const ADVANCED_NUMERIC: ParamSpec[] = [
  ['tts_max_batch_size', 'TTS Batch Size', 0, 10],
  ['codec_max_batch_size', 'Codec Batch Size', 0, 10],
  ['text_temperature', 'Text Temp', 0.1, 2.0],
  ['text_top_p', 'Text Top P', 0.1, 1.0],
  ['text_top_k', 'Text Top K', 1, 100],
  ['audio_top_p', 'Audio Top P', 0.1, 1.0],
  ['audio_top_k', 'Audio Top K', 1, 100],
  ['audio_repetition_penalty', 'Audio Rep Penalty', 1.0, 2.0],
]
const FLOAT_KEYS = new Set<keyof TtsGenerationParams>([
  'text_temperature',
  'audio_temperature',
  'text_top_p',
  'audio_top_p',
  'audio_repetition_penalty',
])

function ParamGrid({
  specs,
  params,
  update,
}: {
  specs: ParamSpec[]
  params: TtsGenerationParams
  update: (k: keyof TtsGenerationParams, v: number | boolean | null) => void
}) {
  return (
    <div className="grid grid-cols-2 gap-x-4 gap-y-2 text-[11px]">
      {specs.map(([key, label, min, max]) => (
        <div key={key} className="flex flex-col gap-0.5">
          <label className="text-[10px] text-muted-foreground">{label}</label>
          <input
            type="number"
            min={min}
            max={max}
            step={FLOAT_KEYS.has(key) ? 0.1 : 1}
            value={params[key] as number}
            onChange={(e) => update(key, parseFloat(e.target.value) || 0)}
            className="h-6 rounded-lg border border-black/[0.09] bg-black/[0.025] px-2 font-mono text-[10px] outline-none focus:border-jade/40"
          />
        </div>
      ))}
    </div>
  )
}

function GenerationOptionsPanel({ params, onChange }: GenerationOptionsPanelProps) {
  const [advancedOpen, setAdvancedOpen] = useState(false)

  const update = (key: keyof TtsGenerationParams, value: number | boolean | null) => {
    onChange({ ...params, [key]: value })
  }

  // Phase TTS-D.3：根据 max_new_frames 实时算时长预估
  const estimate = estimateGeneration(params)

  return (
    <div className="flex flex-col gap-3">
      {/* Phase TTS-D.3：时长 / 首音延迟预估横条 */}
      <div className="flex items-center gap-3 rounded-lg bg-black/[0.025] px-2.5 py-1.5 text-[10px] text-black/60">
        <span className="font-semibold uppercase tracking-wider text-black/40">预估</span>
        <span>
          时长 ≤ <span className="font-mono text-foreground/80">{estimate.maxSeconds.toFixed(1)}s</span>
        </span>
        <span className="text-black/25">·</span>
        <span>
          典型 <span className="font-mono text-foreground/80">{estimate.expectedSeconds.toFixed(1)}s</span>
        </span>
        <span className="text-black/25">·</span>
        <span>
          首音 <span className="font-mono text-foreground/80">~{estimate.firstAudioMs}ms</span>
        </span>
      </div>

      {/* Basic — 总是展开 */}
      <div>
        <div className="mb-2 text-[10px] font-semibold uppercase tracking-widest text-black/35">
          Basic
        </div>
        <ParamGrid specs={BASIC_NUMERIC} params={params} update={update} />
      </div>

      {/* Toggles + Seed */}
      <div className="grid grid-cols-2 gap-x-4 gap-y-2 text-[11px]">
        <div className="flex flex-col gap-0.5">
          <label className="text-[10px] text-muted-foreground">Seed (0=random)</label>
          <input
            type="number"
            value={params.seed ?? 0}
            onChange={(e) => {
              const v = parseInt(e.target.value)
              update('seed', v === 0 ? null : v)
            }}
            className="h-6 rounded-lg border border-black/[0.09] bg-black/[0.025] px-2 font-mono text-[10px] outline-none focus:border-jade/40"
          />
        </div>
        {(
          [
            ['do_sample', 'Do Sample'],
            ['enable_robust_normalization', 'Robust Normalization'],
          ] as [keyof TtsGenerationParams, string][]
        ).map(([key, label]) => (
          <label key={key} className="flex items-center gap-1.5">
            <input
              type="checkbox"
              checked={params[key] as boolean}
              onChange={(e) => update(key, e.target.checked)}
              className="h-3.5 w-3.5 rounded border-black/20 text-jade focus:ring-jade/30"
            />
            <span className="text-[10px] text-muted-foreground">{label}</span>
          </label>
        ))}
      </div>

      {/* Advanced — 默认折叠 */}
      <details open={advancedOpen} onToggle={(e) => setAdvancedOpen((e.target as HTMLDetailsElement).open)}>
        <summary className="cursor-pointer text-[10px] font-semibold uppercase tracking-widest text-black/35 hover:text-black/55 select-none">
          Advanced (sampling · batching)
        </summary>
        <div className="mt-2">
          <ParamGrid specs={ADVANCED_NUMERIC} params={params} update={update} />
        </div>
      </details>
    </div>
  )
}

// ── Playback Script (sentence highlighting) ──────────────────────────────────

function PlaybackScript({
  chunks,
  activeIndex,
}: {
  chunks: string[]
  activeIndex: number | null
}) {
  if (chunks.length === 0) return null
  return (
    <div className="flex flex-wrap gap-1 text-[11px] leading-5">
      {chunks.map((chunk, i) => (
        <span
          key={i}
          className={`rounded-md px-1.5 py-0.5 transition-colors ${
            i === activeIndex
              ? 'bg-jade/15 text-jade font-semibold'
              : i < (activeIndex ?? 0)
                ? 'text-muted-foreground/50'
                : 'text-foreground/70'
          }`}
        >
          {chunk}
        </span>
      ))}
    </div>
  )
}

// ── Phase TTS-D.1: Provider state badge ─────────────────────────────────────

function ProviderStateBadge({ state }: { state: TtsProviderState | null }) {
  if (!state) return null
  const cfg = (() => {
    switch (state.kind) {
      case 'notLoaded':
        return { label: '未加载', cls: 'bg-gray-50 text-gray-600 border-gray-200/70', icon: null }
      case 'loading':
        return {
          label: '加载中…（首次约 3 秒）',
          cls: 'bg-amber-50 text-amber-700 border-amber-200/70',
          icon: <Loader2 className="h-3 w-3 animate-spin" />,
        }
      case 'loaded':
        return {
          label: `已就绪 · ${Math.round(state.elapsedSeconds)}s`,
          cls: 'bg-emerald-50 text-emerald-700 border-emerald-200/70',
          icon: <CheckCircle className="h-3 w-3" />,
        }
      case 'failed':
        return {
          label: `加载失败：${state.error.slice(0, 80)}`,
          cls: 'bg-rose-50 text-rose-700 border-rose-200/70',
          icon: <AlertCircle className="h-3 w-3" />,
        }
      case 'evicted':
        return {
          label: `已释放（空闲 ${Math.round(state.elapsedSeconds)}s）— 下次请求会重新加载`,
          cls: 'bg-violet-50 text-violet-700 border-violet-200/70',
          icon: null,
        }
    }
  })()
  return (
    <div className={`flex items-center gap-2 rounded-xl border px-3 py-1.5 text-[10.5px] ${cfg.cls}`}>
      {cfg.icon}
      <span className="flex-1">{cfg.label}</span>
    </div>
  )
}

// ── Phase TTS-D.3: 生成时长预估 ────────────────────────────────────────────
//
// MOSS-TTS-Nano 12.5 frames/s → max_new_frames / 12.5 = 最长可能时长。
// 通常实际生成在到达 audio_end_token 时提前结束，约 60-80% of 上限。
// 首音延迟在 M-series CPU 4 核典型 ~0.3-0.6s。
function estimateGeneration(params: TtsGenerationParams): {
  maxSeconds: number
  expectedSeconds: number
  firstAudioMs: number
} {
  const maxSeconds = params.max_new_frames / 12.5
  return {
    maxSeconds,
    expectedSeconds: maxSeconds * 0.7,
    firstAudioMs: 400, // 经验值：M4 4 核 prefill + 首帧 codec ≈ 400ms
  }
}

// ── Main Page ────────────────────────────────────────────────────────────────

export function TtsTestPage() {
  // Status
  const [healthState, setHealthState] = useState<'ready' | 'initializing' | 'failed' | 'idle'>('idle')
  const [healthMessage, setHealthMessage] = useState('')
  const [warmupProgress, setWarmupProgress] = useState(0)
  const [runStatus, setRunStatus] = useState('Idle.')
  const [normalizedText, setNormalizedText] = useState('')

  // 只读模型 ready 状态；下载入口在「模型配置」页（TtsModelSection）
  const [modelsReady, setModelsReady] = useState<boolean | null>(null)
  // Phase TTS-D.1：Provider 生命周期状态
  const [providerState, setProviderState] = useState<TtsProviderState | null>(null)

  // Demo selection
  const [selectedDemoId, setSelectedDemoId] = useState('demo-0')

  // Input
  const [text, setText] = useState(DEMOS[0].text)
  const [textChunks, setTextChunks] = useState<string[]>([])

  // Parameters
  const [params, setParams] = useState<TtsGenerationParams>(TTS_DEFAULT_PARAMS)

  // Playback
  const [isGenerating, setIsGenerating] = useState(false)
  const [isPaused, setIsPaused] = useState(false)
  const [bufferedAudioUrl, setBufferedAudioUrl] = useState<string | null>(null)
  const [audioDuration, setAudioDuration] = useState<number | null>(null)
  const [activeChunkIndex, setActiveChunkIndex] = useState<number | null>(null)
  const [currentStreamId, setCurrentStreamId] = useState<string | null>(null)
  const [streamMetrics, setStreamMetrics] = useState<string | null>(null)

  const audioRef = useRef<HTMLAudioElement | null>(null)
  const pollTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const mountedRef = useRef(true)

  // Phase TTS-B.2: Web Audio gapless streaming player
  const webAudio = useWebAudioStreamPlayer()

  // Track mount status to abort async polling on unmount
  useEffect(() => {
    mountedRef.current = true
    return () => {
      mountedRef.current = false
      if (pollTimerRef.current) clearTimeout(pollTimerRef.current)
    }
  }, [])

  // ── Poll warmup status ──────────────────────────────────────────────────

  const pollWarmup = useCallback(async () => {
    try {
      // Phase TTS-D.1：用 ttsHealth 一次拿到 warmup + provider state
      const health = await ttsHealth()
      setProviderState(health.provider_state)
      const status = await ttsWarmupStatus()
      if (status.state === 'ready') {
        setHealthState('ready')
        setHealthMessage(status.message)
        setWarmupProgress(1.0)
      } else if (status.state === 'failed') {
        setHealthState('failed')
        setHealthMessage(status.error || status.message)
        setWarmupProgress(0)
      } else {
        setHealthState('initializing')
        setHealthMessage(status.message)
        setWarmupProgress(status.progress)
      }
    } catch {
      // TTS not initialized or backend not ready
    }
  }, [])

  useEffect(() => {
    void pollWarmup()
    const interval = setInterval(pollWarmup, 2000)
    return () => clearInterval(interval)
  }, [pollWarmup])

  // ── Poll TTS model status ─────────────────────────────────────────────

  const pollModelStatus = useCallback(async () => {
    try {
      const status = await ttsModelStatus()
      setModelsReady(status.ready)
    } catch {
      setModelsReady(null)
    }
  }, [])

  useEffect(() => {
    void pollModelStatus()
    const interval = setInterval(() => {
      void pollModelStatus()
    }, 2000)
    return () => clearInterval(interval)
  }, [pollModelStatus])

  // ── Demo selection change ───────────────────────────────────────────────

  const handleDemoChange = useCallback((demoId: string) => {
    setSelectedDemoId(demoId)
    const demo = DEMOS.find((d) => d.id === demoId)
    if (demo) {
      setText(demo.text)
    }
  }, [])

  // ── Start warmup ────────────────────────────────────────────────────────

  const handleStartWarmup = useCallback(async () => {
    try {
      await ttsStartWarmup()
      setHealthState('initializing')
      setHealthMessage('Starting warmup...')
      toast.info('TTS warmup started')
    } catch (e) {
      toast.error('Warmup failed', { description: String(e) })
      setHealthState('failed')
    }
  }, [])

  // ── Buffered synthesis (Generate button) ────────────────────────────────

  const handleGenerate = useCallback(async () => {
    if (!text.trim()) {
      toast.error('Text is empty')
      return
    }
    setIsGenerating(true)
    setBufferedAudioUrl(null)
    setAudioDuration(null)
    setActiveChunkIndex(null)
    setNormalizedText('')

    // Phase TTS-D.2：lazy reload UI 反馈
    const needsLoad =
      providerState?.kind === 'notLoaded' || providerState?.kind === 'evicted'
    if (needsLoad) {
      setRunStatus('正在加载模型 (~3s)...')
      toast.info('TTS 模型正在加载', {
        description: '首次或空闲驱逐后第一次合成需要约 3 秒加载 ONNX sessions',
      })
    } else {
      setRunStatus('Generating...')
    }

    try {
      const demoId = selectedDemoId
      const voiceId = getAgentVoiceId() // 优先用 agent voice
      const result = await ttsSynthesize(text, voiceId ? null : demoId, null, params, voiceId)
      setRunStatus(`Done. voice=${result.voice} duration=${result.duration_seconds.toFixed(1)}s`)

      // Create blob URL from base64 WAV
      const byteCharacters = atob(result.audio_base64)
      const byteNumbers = new Array(byteCharacters.length)
      for (let i = 0; i < byteCharacters.length; i++) {
        byteNumbers[i] = byteCharacters.charCodeAt(i)
      }
      const byteArray = new Uint8Array(byteNumbers)
      const blob = new Blob([byteArray], { type: 'audio/wav' })
      const url = URL.createObjectURL(blob)
      setBufferedAudioUrl(url)
      setAudioDuration(result.duration_seconds)
      setTextChunks(result.text_chunks)
      setNormalizedText(text) // In a real implementation, this would be the normalized text

      toast.success('Synthesis complete', { description: `Duration: ${result.duration_seconds.toFixed(1)}s` })
    } catch (e) {
      setRunStatus(`Error: ${e}`)
      toast.error('Synthesis failed', { description: String(e) })
    } finally {
      setIsGenerating(false)
    }
  }, [text, selectedDemoId, params])

  // ── Streaming synthesis ─────────────────────────────────────────────────

  const handleStream = useCallback(async () => {
    if (!text.trim()) {
      toast.error('Text is empty')
      return
    }
    setIsGenerating(true)
    setBufferedAudioUrl(null)
    setAudioDuration(null)
    setActiveChunkIndex(null)
    setTextChunks([])
    setStreamMetrics(null)

    // Phase TTS-D.2：lazy reload UI 反馈
    const needsLoad =
      providerState?.kind === 'notLoaded' || providerState?.kind === 'evicted'
    if (needsLoad) {
      setRunStatus('正在加载模型 (~3s)...')
      toast.info('TTS 模型正在加载', {
        description: '首次或空闲驱逐后第一次合成需要约 3 秒加载 ONNX sessions',
      })
    } else {
      setRunStatus('Starting stream...')
    }

    try {
      // Phase TTS-C.4：流式启动前先拿真实 chunks 用于高亮 / 预览
      try {
        const splitChunks = await ttsSplitText(text, params.voice_clone_max_text_tokens)
        setTextChunks(splitChunks.length > 0 ? splitChunks : [text])
      } catch {
        setTextChunks([text])
      }

      // Phase TTS-B.2: 启动 Web Audio gapless 播放器（订阅 tts:stream-chunk）
      await webAudio.start()

      const demoId = selectedDemoId
      const voiceId = getAgentVoiceId()
      const result = await ttsStreamStart(text, voiceId ? null : demoId, null, params, voiceId)
      setCurrentStreamId(result.stream_id)
      webAudio.setExpectedStreamId(result.stream_id)
      setRunStatus(`Streaming (id=${result.stream_id.slice(0, 8)}...)`)

      // Poll status until done
      const poll = async () => {
        if (!mountedRef.current) return
        try {
          const status = await ttsStreamStatus(result.stream_id)
          const state = (status.state as string) || ''
          const emitted = (status.emitted_audio_seconds as number) ?? 0
          const lead = (status.lead_seconds as number) ?? 0
          const firstAudioSecs = status.first_audio_latency_seconds as number | null
          const rtf = status.realtime_factor as number | null
          const chunkIdx = status.current_chunk_index as number | null
          // 优先使用 Web Audio 客户端测量的首音延迟（更准）
          const firstMs = webAudio.metrics.firstAudioLatencyMs
            ?? (firstAudioSecs != null ? Math.round(firstAudioSecs * 1000) : null)
          const parts = [
            `emitted=${emitted.toFixed(1)}s`,
            `lead=${lead.toFixed(2)}s`,
          ]
          if (firstMs !== null) parts.push(`first_audio=${firstMs}ms`)
          if (rtf != null && rtf > 0) parts.push(`RTF=${rtf.toFixed(2)}x`)
          setStreamMetrics(parts.join(' · '))
          setActiveChunkIndex(chunkIdx)

          if (state === 'done' || state === 'failed' || state === 'closed') {
            setRunStatus(state === 'done' ? 'Stream complete.' : `Stream ${state}.`)
            setIsGenerating(false)
            setCurrentStreamId(null)
            // 别立即 stop()，让 audio buffer 自然 drain；3s 后清理
            setTimeout(() => { void webAudio.stop() }, 3000)
            return
          }
          pollTimerRef.current = setTimeout(poll, 200)
        } catch {
          if (!mountedRef.current) return
          setIsGenerating(false)
          setCurrentStreamId(null)
          void webAudio.stop()
        }
      }
      poll()
    } catch (e) {
      setRunStatus(`Stream error: ${e}`)
      toast.error('Stream failed', { description: String(e) })
      setIsGenerating(false)
      void webAudio.stop()
    }
  }, [text, selectedDemoId, params, webAudio])

  // ── Stop stream ─────────────────────────────────────────────────────────

  const handleStop = useCallback(async () => {
    if (currentStreamId) {
      try {
        await ttsStreamClose(currentStreamId)
        setRunStatus('Stream stopped.')
        setCurrentStreamId(null)
      } catch {
        // ignore
      }
    }
    setIsGenerating(false)
    if (pollTimerRef.current) {
      clearTimeout(pollTimerRef.current)
      pollTimerRef.current = null
    }
    void webAudio.stop()
  }, [currentStreamId, webAudio])

  // ── Pause/Resume：同时控制 buffered <audio> 和 streaming Web Audio ──────

  const handlePauseResume = useCallback(async () => {
    // 流式播放优先（active 时）
    if (webAudio.isActive) {
      if (webAudio.isPaused) {
        await webAudio.resume()
        setIsPaused(false)
      } else {
        await webAudio.pause()
        setIsPaused(true)
      }
      return
    }
    // 否则走 buffered <audio>
    if (audioRef.current) {
      if (audioRef.current.paused) {
        await audioRef.current.play()
        setIsPaused(false)
      } else {
        audioRef.current.pause()
        setIsPaused(true)
      }
    }
  }, [webAudio])

  // ── Audio time update → highlight active chunk ──────────────────────────
  //
  // Phase TTS-C.4：按 chunk **字符长度加权**找到当前位置（之前是平均切分，长短
  // 不齐时跑偏）。比如 ['你好。', '这是一段较长的文本测试用。'] 字符比是
  // 1:8，进度 0.5 时旧逻辑高亮 chunk[1]，新逻辑高亮 chunk[0]→后段一会切到 [1]。
  const handleAudioTimeUpdate = useCallback(() => {
    if (!audioRef.current || textChunks.length === 0) return
    const ratio = audioRef.current.currentTime / (audioRef.current.duration || 1)
    const totalChars = textChunks.reduce((a, c) => a + c.length, 0) || 1
    const target = ratio * totalChars
    let acc = 0
    for (let i = 0; i < textChunks.length; i++) {
      acc += textChunks[i].length
      if (target <= acc) {
        setActiveChunkIndex(i)
        return
      }
    }
    setActiveChunkIndex(textChunks.length - 1)
  }, [textChunks])

  // ── Cleanup on unmount ──────────────────────────────────────────────────

  useEffect(() => {
    return () => {
      if (pollTimerRef.current) clearTimeout(pollTimerRef.current)
      if (bufferedAudioUrl) URL.revokeObjectURL(bufferedAudioUrl)
    }
  }, [bufferedAudioUrl])

  // ── Selected demo info ──────────────────────────────────────────────────

  const selectedDemo = DEMOS.find((d) => d.id === selectedDemoId)

  return (
    <div className="flex flex-col gap-3">
      {/* ── Header ── */}
      <SettingsSurface className="px-5 py-4">
        <SectionLabel>MOSS-TTS-Nano Demo</SectionLabel>
        <p className="mb-1 text-[11.5px] leading-4 text-muted-foreground">
          State-of-the-art text-to-speech for multilingual voice cloning.
        </p>
        <div className="flex flex-wrap gap-1 text-[10px] text-muted-foreground/70">
          <span>Voice Clone</span>
          <span>•</span>
          <span>Streaming</span>
          <span>•</span>
          <span>20 Languages</span>
          <span>•</span>
          <span>15 Voice Presets</span>
        </div>
      </SettingsSurface>

      {/* ── Status row ── */}
      <div className="flex flex-col gap-1.5">
        <StatusBadge state={healthState} progress={warmupProgress} message={healthMessage} />
        <ProviderStateBadge state={providerState} />
        <div className="flex items-center gap-2 rounded-xl border border-black/[0.07] bg-black/[0.016] px-3 py-2 text-[11px] text-foreground/60">
          <span className="font-semibold text-black/30">Run Status:</span>
          <span className="flex-1 truncate">{runStatus}</span>
          {streamMetrics && (
            <span className="font-mono text-[10px] text-jade">{streamMetrics}</span>
          )}
        </div>
      </div>

      {/* ── Model not downloaded → 引导去「模型配置」页 ── */}
      {modelsReady === false && (
        <SettingsSurface className="overflow-visible px-5 py-3">
          <div className="flex items-center gap-3 text-[11px]">
            <AlertCircle className="size-4 shrink-0 text-amber-600" />
            <span className="flex-1 text-muted-foreground">
              TTS 模型未下载。请前往「<span className="font-semibold text-foreground/80">模型配置</span>」页面下载（约 830MB），下载后重启应用即可使用真实语音合成。
            </span>
          </div>
        </SettingsSurface>
      )}

      {/* ── Two-column layout: Input | Output ── */}
      <div className="grid grid-cols-1 gap-3 lg:grid-cols-2">
        {/* ── INPUT PANEL ── */}
        <div className="flex flex-col gap-3">
          {/* Demo selector */}
          <SettingsSurface className="px-5 py-4">
            <SectionLabel>Demo</SectionLabel>
            <div className="relative">
              <select
                value={selectedDemoId}
                onChange={(e) => handleDemoChange(e.target.value)}
                className="w-full appearance-none rounded-xl border border-black/9 bg-black/2.5 px-3 py-2 text-[12px] text-foreground/75 outline-none transition-colors focus:border-jade/40 focus:ring-[3px] focus:ring-jade/15"
                style={{ cursor: 'pointer' }}
              >
                {DEMOS.map((d) => (
                  <option key={d.id} value={d.id}>
                    {d.label}
                  </option>
                ))}
              </select>
              <div className="pointer-events-none absolute right-2 top-1/2 -translate-y-1/2 text-black/30">
                <svg width="9" height="5" viewBox="0 0 10 6" fill="none">
                  <path d="M1 1l4 4 4-4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
                </svg>
              </div>
            </div>
            {selectedDemo && (
              <p className="mt-2 text-[10px] text-muted-foreground">
                Prompt voice: {selectedDemo.role}.wav
              </p>
            )}
          </SettingsSurface>

          {/* Text input */}
          <SettingsSurface className="px-5 py-4">
            <SectionLabel>Text</SectionLabel>
            <textarea
              value={text}
              onChange={(e) => setText(e.target.value)}
              rows={4}
              className="w-full resize-none rounded-xl border border-black/9 bg-black/2.5 px-3 py-2 text-[12px] leading-5 text-foreground/80 outline-none transition-colors focus:border-jade/40 focus:ring-[3px] focus:ring-jade/15"
              placeholder="Enter text to synthesize..."
            />
          </SettingsSurface>

          {/* Generation Options */}
          <SettingsSurface className="px-5 py-4">
            <GenerationOptionsPanel params={params} onChange={setParams} />
          </SettingsSurface>

          {/* Action buttons */}
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={() => handleGenerate()}
              disabled={isGenerating || !text.trim() || modelsReady !== true}
              className="flex-1 flex items-center justify-center gap-1.5 rounded-xl bg-jade px-4 py-2 text-[12px] font-semibold text-white shadow-sm transition-all hover:bg-jade/90 disabled:cursor-not-allowed disabled:opacity-40"
              title={modelsReady === false ? '请先下载 TTS 模型' : undefined}
            >
              {isGenerating ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <Play className="h-4 w-4" />
              )}
              Generate (Buffered)
            </button>
            <button
              type="button"
              onClick={() => void handleStream()}
              disabled={isGenerating || !text.trim() || modelsReady !== true}
              className="flex items-center justify-center gap-1.5 rounded-xl border border-black/9 bg-white px-3 py-2 text-[12px] font-semibold text-black/60 shadow-sm transition-all hover:text-black disabled:cursor-not-allowed disabled:opacity-40"
              title={modelsReady === false ? '请先下载 TTS 模型' : undefined}
            >
              Stream
            </button>
            {isGenerating && currentStreamId && (
              <button
                type="button"
                onClick={handleStop}
                className="flex items-center justify-center gap-1.5 rounded-xl border border-black/9 bg-white px-3 py-2 text-[12px] font-semibold text-black/60 shadow-sm transition-all hover:text-black"
              >
                <Pause className="h-4 w-4" />
                Stop
              </button>
            )}
            {(bufferedAudioUrl || webAudio.isActive) && (
              <button
                type="button"
                onClick={() => void handlePauseResume()}
                className="flex items-center justify-center gap-1.5 rounded-xl border border-black/9 bg-white px-3 py-2 text-[12px] font-semibold text-black/60 shadow-sm transition-all hover:text-black"
                title={webAudio.isActive ? 'Pause/Resume streaming' : 'Pause/Resume buffered'}
              >
                {isPaused ? <Play className="h-4 w-4" /> : <Pause className="h-4 w-4" />}
              </button>
            )}
            <button
              type="button"
              onClick={() => void handleStartWarmup()}
              className="flex items-center justify-center rounded-xl border border-black/9 bg-white p-2 text-black/40 shadow-sm transition-all hover:text-black/70"
              title="Refresh Warmup"
            >
              <RefreshCw className="h-4 w-4" />
            </button>
          </div>
        </div>

        {/* ── OUTPUT PANEL ── */}
        <div className="flex flex-col gap-3">
          {/* Phase TTS-D / P0：Agent Voice Picker */}
          <SettingsSurface className="px-5 py-4">
            <AgentVoicePicker />
          </SettingsSurface>

          {/* Normalized Text */}
          <SettingsSurface className="px-5 py-4">
            <SectionLabel>Normalized Text</SectionLabel>
            <div className="rounded-xl border border-black/6 bg-black/1.5 px-3 py-2 text-[12px] leading-5 text-foreground/70 min-h-10">
              {normalizedText || <span className="text-muted-foreground/40">Waiting for synthesis...</span>}
            </div>
          </SettingsSurface>

          {/* Playback Script */}
          <SettingsSurface className="px-5 py-4">
            <SectionLabel>Playback Script</SectionLabel>
            <PlaybackScript chunks={textChunks} activeIndex={activeChunkIndex} />
            {textChunks.length === 0 && (
              <span className="text-[11px] text-muted-foreground/40">
                Synthesized chunks will appear here.
              </span>
            )}
          </SettingsSurface>

          {/* Generated Speech / Audio Player */}
          <SettingsSurface className="px-5 py-4">
            <SectionLabel>Generated Speech</SectionLabel>
            {bufferedAudioUrl ? (
              <div className="flex flex-col gap-2">
                <audio
                  ref={audioRef}
                  src={bufferedAudioUrl}
                  controls
                  className="w-full"
                  onTimeUpdate={handleAudioTimeUpdate}
                  onEnded={() => setActiveChunkIndex(null)}
                />
                {audioDuration && (
                  <p className="text-[10px] text-muted-foreground">
                    Duration: {audioDuration.toFixed(1)}s | Sample Rate: 48000Hz | Stereo
                  </p>
                )}
              </div>
            ) : (
              <div className="rounded-xl border border-black/6 bg-black/1.5 px-3 py-6 text-center text-[11px] text-muted-foreground/40">
                {isGenerating ? (
                  <div className="flex items-center justify-center gap-2">
                    <Loader2 className="h-4 w-4 animate-spin" />
                    <span>Synthesizing...</span>
                  </div>
                ) : (
                  'Click Generate to synthesize speech.'
                )}
              </div>
            )}
          </SettingsSurface>

          {/* Model Info */}
          <SettingsSurface className="px-5 py-4">
            <SectionLabel>Model Info</SectionLabel>
            <div className="flex flex-col gap-1 text-[10px] text-muted-foreground">
              <p>
                <span className="font-semibold text-black/30">Checkpoint:</span>{' '}
                ~/.if2ai/models/tts/
              </p>
              <p>
                <span className="font-semibold text-black/30">Audio Tokenizer:</span>{' '}
                ~/.if2ai/models/tts/audio_tokenizer/
              </p>
              <p>
                <span className="font-semibold text-black/30">Runtime:</span>{' '}
                ONNX Runtime CPU (ort crate 2.0)
              </p>
            </div>
          </SettingsSurface>
        </div>
      </div>
    </div>
  )
}
