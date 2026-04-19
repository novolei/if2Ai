/**
 * SttButton — 聊天输入栏的语音输入按钮（Whisper STT）。
 *
 * 流程：
 * 1. 用户长按（或点击一次切换）→ 启动 MediaRecorder 录音
 * 2. 停止录音 → PCM16LE base64 → `stt_transcribe` Tauri 命令
 * 3. 转写结果插入输入框（`onTranscribe(text)`）
 *
 * 需要浏览器麦克风权限（Tauri v2 默认允许 WebView 请求麦克风）。
 *
 * 当 whisper 模型未下载时，按钮变灰并 tooltip 提示下载路径。
 */

import { useCallback, useEffect, useRef, useState } from 'react'
import { Mic, MicOff, Loader2 } from 'lucide-react'
import { toast } from 'sonner'
import {
  sttModelStatus,
  sttGetSettings,
  sttTranscribe,
  type SttSettingsDto,
} from '@/lib/tauri'
import { RecordingIndicator } from './RecordingIndicator'
import { useCrossWindowChange } from '@/lib/crossWindowSync'

interface Props {
  /** 转写成功后的回调（把文本插入输入框）。 */
  onTranscribe: (text: string) => void
  disabled?: boolean
}

type RecordState = 'idle' | 'recording' | 'processing' | 'error'

/** PCM16LE 提取：把 MediaRecorder WebM/OGG 输出转成 PCM16LE (16kHz mono)。 */
async function audioBlobToPcm16leBase64(blob: Blob): Promise<string> {
  const audioCtx = new AudioContext({ sampleRate: 16_000 })
  const arrayBuffer = await blob.arrayBuffer()
  const decoded = await audioCtx.decodeAudioData(arrayBuffer)
  // 取第一声道（mono）
  const raw = decoded.getChannelData(0)
  // f32 → PCM16LE
  const pcm16 = new Int16Array(raw.length)
  for (let i = 0; i < raw.length; i++) {
    const clamped = Math.max(-1, Math.min(1, raw[i]))
    pcm16[i] = Math.round(clamped * 32767)
  }
  await audioCtx.close()
  // Int16Array → Uint8Array → base64
  const uint8 = new Uint8Array(pcm16.buffer)
  let binary = ''
  const chunkSize = 0x8000
  for (let i = 0; i < uint8.length; i += chunkSize) {
    const sub = uint8.subarray(i, Math.min(i + chunkSize, uint8.length))
    binary += String.fromCharCode.apply(null, Array.from(sub))
  }
  return btoa(binary)
}

export function SttButton({ onTranscribe, disabled = false }: Props) {
  const [modelReady, setModelReady] = useState<boolean | null>(null)
  const [openflowReady, setOpenflowReady] = useState<boolean | null>(null)
  const [settings, setSettings] = useState<SttSettingsDto | null>(null)
  const [downloadHint, setDownloadHint] = useState('')
  const [recordState, setRecordState] = useState<RecordState>('idle')
  const recorderRef = useRef<MediaRecorder | null>(null)
  const chunksRef = useRef<Blob[]>([])

  // 加载状态：whisper 模型 + STT settings（决定当前 provider）
  const refreshState = useCallback(async () => {
    try {
      const [s, m] = await Promise.all([sttGetSettings(), sttModelStatus()])
      setSettings(s)
      setModelReady(m.ready)
      setOpenflowReady(m.openflow_ready)
      setDownloadHint(m.download_hint)
    } catch {
      setModelReady(false)
      setOpenflowReady(false)
    }
  }, [])
  useEffect(() => { void refreshState() }, [refreshState])

  // 跨窗口同步：Settings 改 STT provider / 下载完模型后立即重新拉状态
  useCrossWindowChange('cross:stt-settings-changed', () => {
    void refreshState()
  })

  // 当前 provider 是否可用？
  const provider = settings?.provider ?? 'whisper'
  const providerReady = provider === 'groq'
    ? !!settings?.groq_api_key_set
    : provider === 'openflow'
      ? openflowReady === true
      : modelReady === true

  const stopRecording = useCallback(async () => {
    const rec = recorderRef.current
    if (!rec || rec.state === 'inactive') return
    rec.stop()
  }, [])

  const startRecording = useCallback(async () => {
    if (!providerReady) {
      // 引导用户去设置页
      toast.error('STT 未配置', {
        description: provider === 'groq'
          ? '请去 设置 → STT 语音输入 → 填 Groq API Key'
          : provider === 'openflow'
            ? '请去 设置 → STT 语音输入 → 下载 SenseVoice 模型（230MB）'
            : '请去 设置 → STT 语音输入 → 下载 Whisper 模型',
        duration: 4000,
      })
      return
    }
    try {
      const stream = await navigator.mediaDevices.getUserMedia({ audio: true, video: false })
      const rec = new MediaRecorder(stream)
      chunksRef.current = []
      rec.ondataavailable = (e) => {
        if (e.data.size > 0) chunksRef.current.push(e.data)
      }
      rec.onstop = async () => {
        stream.getTracks().forEach((t) => t.stop())
        setRecordState('processing')
        try {
          const blob = new Blob(chunksRef.current, { type: rec.mimeType })
          const base64 = await audioBlobToPcm16leBase64(blob)
          const result = await sttTranscribe({
            audio_bytes_base64: base64,
            language: null,
            sample_rate: 16_000,
          })
          if (result.text.trim()) {
            onTranscribe(result.text.trim())
            toast.success('转写完成', {
              description: `${result.provider} · ${result.elapsed_seconds.toFixed(1)}s`,
              duration: 1800,
            })
          } else {
            toast.info('未识别到语音内容')
          }
        } catch (e) {
          console.error('[STT] transcribe failed:', e)
          toast.error('转写失败', { description: String(e) })
          setRecordState('error')
          setTimeout(() => setRecordState('idle'), 1500)
          return
        }
        setRecordState('idle')
      }
      recorderRef.current = rec
      rec.start(100)
      setRecordState('recording')
    } catch (e) {
      console.error('[STT] getUserMedia failed:', e)
      toast.error('麦克风访问失败', { description: '请在系统设置中授权麦克风权限' })
      setRecordState('error')
      setTimeout(() => setRecordState('idle'), 1500)
    }
  }, [providerReady, provider, onTranscribe])

  const handleClick = useCallback(() => {
    if (recordState === 'recording') {
      void stopRecording()
    } else if (recordState === 'idle') {
      void startRecording()
    }
  }, [recordState, startRecording, stopRecording])

  // tooltip 文案
  const providerLabel = provider === 'groq'
    ? 'Groq Whisper API'
    : provider === 'openflow'
      ? 'SenseVoice 本地'
      : 'Whisper 本地'
  const title = !providerReady
    ? provider === 'groq'
      ? 'Groq API Key 未配置（点击查看提示）'
      : provider === 'openflow'
        ? 'SenseVoice 模型未下载（点击查看提示）'
        : `Whisper 模型未下载\n${downloadHint}`
    : recordState === 'recording' ? '点击停止录音'
    : recordState === 'processing' ? '转写中…'
    : recordState === 'error' ? '转写失败，请重试'
    : `点击开始语音输入（${providerLabel}）`

  // 关键修复：未 ready 时 button 不 disabled，让用户点击触发 toast 引导（之前是 disabled 完全无反应）
  const isDisabled = disabled || recordState === 'processing'

  return (
    <>
      <button
        type="button"
        onClick={handleClick}
        disabled={isDisabled}
        title={title}
        className={`flex items-center justify-center rounded-xl p-1.5 transition-colors disabled:opacity-40 ${
          recordState === 'recording'
            ? 'bg-rose-500/15 text-rose-600 hover:bg-rose-500/20'
            : recordState === 'error'
              ? 'text-rose-500'
              : 'text-black/45 hover:bg-black/[0.04] hover:text-black/70'
        }`}
      >
        {recordState === 'processing' ? (
          <Loader2 className="size-4 animate-spin" />
        ) : recordState === 'recording' ? (
          <div className="relative">
            <span className="absolute inset-0 animate-ping rounded-full bg-rose-500/40" />
            <MicOff className="relative size-4" />
          </div>
        ) : (
          <Mic className="size-4" />
        )}
      </button>
      {(recordState === 'recording' || recordState === 'processing') && (
        <RecordingIndicator state={recordState} providerLabel={providerLabel} />
      )}
    </>
  )
}
