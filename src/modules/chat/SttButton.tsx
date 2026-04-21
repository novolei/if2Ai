/**
 * SttButton — 聊天输入栏的语音输入按钮（OpenFlow / SenseVoice STT）。
 *
 * 流程：
 * 1. 用户点击 → 启动 MediaRecorder 录音
 * 2. 再点击停止 → PCM16LE base64 → `stt_transcribe` Tauri 命令
 * 3. 转写结果插入输入框（`onTranscribe(text)`）
 *
 * 需要 macOS 麦克风权限（首次使用会弹原生授权对话框）。
 *
 * 当 SenseVoice 模型未下载时，按钮仍可点，会弹 toast 引导用户去设置页下载。
 */

import { useCallback, useEffect, useRef, useState } from 'react'
import { Mic, MicOff, Loader2 } from 'lucide-react'
import { toast } from 'sonner'
import { sttModelStatus, sttTranscribe } from '@/lib/tauri'
import { RecordingIndicator } from './RecordingIndicator'
import { useCrossWindowChange } from '@/lib/crossWindowSync'

interface Props {
  /** 转写成功后的回调（把文本插入输入框）。 */
  onTranscribe: (text: string) => void
  disabled?: boolean
}

type RecordState = 'idle' | 'recording' | 'processing' | 'error'

const PROVIDER_LABEL = 'SenseVoice 本地'

/** PCM16LE 提取：把 MediaRecorder WebM/OGG 输出转成 PCM16LE (16kHz mono)。 */
async function audioBlobToPcm16leBase64(blob: Blob): Promise<string> {
  const audioCtx = new AudioContext({ sampleRate: 16_000 })
  const arrayBuffer = await blob.arrayBuffer()
  const decoded = await audioCtx.decodeAudioData(arrayBuffer)
  const raw = decoded.getChannelData(0)
  const pcm16 = new Int16Array(raw.length)
  for (let i = 0; i < raw.length; i++) {
    const clamped = Math.max(-1, Math.min(1, raw[i]))
    pcm16[i] = Math.round(clamped * 32767)
  }
  await audioCtx.close()
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
  const [openflowReady, setOpenflowReady] = useState<boolean | null>(null)
  const [recordState, setRecordState] = useState<RecordState>('idle')
  const recorderRef = useRef<MediaRecorder | null>(null)
  const chunksRef = useRef<Blob[]>([])

  const refreshState = useCallback(async () => {
    try {
      const m = await sttModelStatus()
      setOpenflowReady(m.openflow_ready)
    } catch {
      setOpenflowReady(false)
    }
  }, [])
  useEffect(() => {
    void refreshState()
  }, [refreshState])

  // 跨窗口同步：Settings 下载完模型后立即刷新状态
  useCrossWindowChange('cross:stt-settings-changed', () => {
    void refreshState()
  })

  const stopRecording = useCallback(async () => {
    const rec = recorderRef.current
    if (!rec || rec.state === 'inactive') return
    rec.stop()
  }, [])

  const startRecording = useCallback(async () => {
    if (openflowReady !== true) {
      toast.error('SenseVoice 模型未下载', {
        description: '请去 设置 → STT 语音输入 → 一键下载（约 230MB）',
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
              description: `${PROVIDER_LABEL} · ${result.elapsed_seconds.toFixed(1)}s`,
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
  }, [openflowReady, onTranscribe])

  const handleClick = useCallback(() => {
    if (recordState === 'recording') {
      void stopRecording()
    } else if (recordState === 'idle') {
      void startRecording()
    }
  }, [recordState, startRecording, stopRecording])

  const title =
    openflowReady === false
      ? 'SenseVoice 模型未下载（点击查看提示）'
      : recordState === 'recording'
        ? '点击停止录音'
        : recordState === 'processing'
          ? '转写中…'
          : recordState === 'error'
            ? '转写失败，请重试'
            : `点击开始语音输入（${PROVIDER_LABEL}）`

  // 未 ready 时仍允许点击，触发 toast 引导（disabled 会让用户摸不着头脑）
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
        <RecordingIndicator state={recordState} providerLabel={PROVIDER_LABEL} />
      )}
    </>
  )
}
