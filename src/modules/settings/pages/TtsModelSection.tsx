/**
 * TtsModelSection — TTS 模型下载管理组件。
 *
 * 嵌入在 Settings → 模型配置 页面底部，显示 TTS 模型状态，
 * 支持一键下载、进度显示、错误处理和重试。
 */

import { useState, useEffect, useCallback } from 'react'
import { toast } from 'sonner'
import { Volume2, Download, CheckCircle, AlertCircle, Loader2, ChevronDown, RefreshCw } from 'lucide-react'
import { SettingsSurface } from '../components/SettingsSurface'
import {
  ttsModelStatus,
  ttsModelDownloadStart,
  ttsModelDownloadStatus,
} from '@/lib/tauri'

// ── Types ─────────────────────────────────────────────────────────────────────

interface ModelFileInfo {
  name: string
  size: number
  present: boolean
}

interface TtsModelStatus {
  ready: boolean
  tts_files: ModelFileInfo[]
  tokenizer_files: ModelFileInfo[]
  total_bytes: number
  missing_bytes: number
  cache_dir: string
}

interface DownloadStatus {
  is_downloading: boolean
  percent: number
  downloaded_bytes: number
  total_bytes: number
  current_file: string
  error: string | null
}

// ── Helpers ───────────────────────────────────────────────────────────────────

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B'
  const mb = bytes / (1024 * 1024)
  if (mb >= 1024) return `${(mb / 1024).toFixed(1)} GB`
  return `${Math.round(mb)} MB`
}

function SectionLabel({ children }: { children: React.ReactNode }) {
  return (
    <div className="mb-3 text-[10.5px] font-semibold uppercase tracking-widest text-black/30">
      {children}
    </div>
  )
}

// ── File list ─────────────────────────────────────────────────────────────────

function FileList({
  label,
  files,
}: {
  label: string
  files: ModelFileInfo[]
}) {
  return (
    <div className="mt-3">
      <p className="text-[10px] font-semibold text-black/35">{label}</p>
      <div className="mt-1 flex flex-col gap-0.5">
        {files.map((f) => (
          <div
            key={f.name}
            className="flex items-center gap-2 text-[10px] text-foreground/60"
          >
            {f.present ? (
              <CheckCircle className="h-3 w-3 shrink-0 text-jade" />
            ) : (
              <AlertCircle className="h-3 w-3 shrink-0 text-black/20" />
            )}
            <span className="truncate">{f.name}</span>
            <span className="ml-auto shrink-0 font-mono text-[9px] text-muted-foreground/60">
              {f.present ? formatBytes(f.size) : '—'}
            </span>
          </div>
        ))}
      </div>
    </div>
  )
}

// ── Progress bar ──────────────────────────────────────────────────────────────

function ProgressBar({
  percent,
  isComplete,
  hasError,
}: {
  percent: number
  isComplete: boolean
  hasError: boolean
}) {
  const clamped = Math.max(0, Math.min(100, percent))
  return (
    <div className="h-2 rounded-full bg-muted overflow-hidden">
      <div
        role="progressbar"
        aria-valuenow={Math.round(clamped)}
        className="h-full rounded-full transition-all duration-300"
        style={{
          width: `${clamped}%`,
          backgroundColor: hasError
            ? '#ef4444'
            : isComplete
              ? '#22c55e'
              : '#06b6d4',
        }}
      />
    </div>
  )
}

// ── Main Component ────────────────────────────────────────────────────────────

export function TtsModelSection() {
  const [status, setStatus] = useState<TtsModelStatus | null>(null)
  const [download, setDownload] = useState<DownloadStatus>({
    is_downloading: false,
    percent: 0,
    downloaded_bytes: 0,
    total_bytes: 0,
    current_file: '',
    error: null,
  })
  const [expanded, setExpanded] = useState(false)
  const [loading, setLoading] = useState(true)

  const loadStatus = useCallback(async () => {
    try {
      const s = await ttsModelStatus()
      setStatus(s)
    } catch {
      setStatus(null)
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    void loadStatus()
  }, [loadStatus])

  // Poll download progress
  useEffect(() => {
    if (!download.is_downloading && !download.error) return

    const poll = async () => {
      try {
        const d = await ttsModelDownloadStatus()
        setDownload(d)

        if (!d.is_downloading) {
          // Download completed or failed
          if (!d.error) {
            toast.success('TTS 模型下载完成', { description: '现在可以返回 TTS 测试页进行语音合成。' })
          }
          void loadStatus()
        }
      } catch {
        // Ignore poll errors, they'll be retried
      }
    }

    const interval = setInterval(poll, 500)
    return () => clearInterval(interval)
  }, [download.is_downloading, download.error, loadStatus])

  const handleDownload = async () => {
    try {
      await ttsModelDownloadStart()
      setDownload({
        is_downloading: true,
        percent: 0,
        downloaded_bytes: 0,
        total_bytes: 0,
        current_file: 'Preparing...',
        error: null,
      })
      toast.info('TTS 模型下载开始', { description: '~700MB，请保持网络连接。' })
    } catch (e) {
      const msg = String(e)
      if (msg.includes('already in progress')) {
        toast.info('下载进行中', { description: '请稍候，正在下载模型文件。' })
      } else {
        toast.error('下载失败', { description: msg })
        setDownload((d) => ({ ...d, error: msg, is_downloading: false }))
      }
    }
  }

  if (loading) {
    return (
      <SettingsSurface className="px-5 py-4">
        <div className="flex items-center gap-2 text-[11px] text-muted-foreground">
          <Loader2 className="h-3.5 w-3.5 animate-spin" />
          检查 TTS 模型状态…
        </div>
      </SettingsSurface>
    )
  }

  const isReady = status?.ready ?? false
  const hasError = !!download.error
  const isDownloading = download.is_downloading

  return (
    <SettingsSurface className="px-5 py-4">
      {/* Header */}
      <div className="flex items-start gap-3">
        <div className="flex size-8 shrink-0 items-center justify-center rounded-xl bg-cyan-500/[0.09]">
          <Volume2 className="size-4 text-cyan-600" />
        </div>

        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <SectionLabel>TTS 语音合成</SectionLabel>
            {isReady && (
              <span className="rounded-xl bg-jade/10 px-2 py-0.5 text-[10px] font-semibold text-jade">
                已安装
              </span>
            )}
            {!isReady && !isDownloading && (
              <span className="rounded-xl bg-black/[0.04] px-2 py-0.5 text-[10px] font-semibold text-black/35">
                未安装
              </span>
            )}
          </div>

          <p className="text-[11.5px] leading-5 text-muted-foreground">
            MOSS-TTS-Nano · 20 语言 · 15 音色 · Voice Clone · Streaming
          </p>
        </div>

        {/* Expand toggle */}
        <button
          type="button"
          onClick={() => setExpanded(!expanded)}
          className="flex h-7 w-7 shrink-0 items-center justify-center rounded-xl border border-black/[0.09] bg-black/[0.025] text-black/35 transition-colors hover:bg-black/[0.05] hover:text-black/60"
          aria-label="展开详情"
        >
          <ChevronDown
            className={`h-3.5 w-3.5 transition-transform ${expanded ? 'rotate-180' : ''}`}
          />
        </button>
      </div>

      {/* Quick status row */}
      <div className="mt-3 flex flex-wrap items-center gap-3 text-[11px] text-foreground/60">
        {!isReady && !isDownloading && (
          <span className="text-amber-600">
            约需下载 {status ? formatBytes(status.missing_bytes) : '~700MB'}
          </span>
        )}
        {isReady && (
          <span className="text-jade">
            安装位置: {status?.cache_dir ?? '~/.if2ai/models/tts/'}
          </span>
        )}
        {isDownloading && (
          <span className="text-cyan-600">
            {download.current_file || '准备中…'} — {Math.round(download.percent * 100)}%
          </span>
        )}
        {hasError && (
          <span className="text-rose-600">{download.error}</span>
        )}
      </div>

      {/* Progress bar (during download) */}
      {isDownloading && (
        <div className="mt-3">
          <ProgressBar
            percent={download.percent * 100}
            isComplete={false}
            hasError={hasError}
          />
          <div className="mt-1 flex items-center justify-between text-[10px] text-muted-foreground">
            <span>{download.current_file}</span>
            <span className="font-mono">
              {formatBytes(download.downloaded_bytes)} / {formatBytes(download.total_bytes ?? status?.total_bytes ?? 0)}
            </span>
          </div>
        </div>
      )}

      {/* Action button */}
      <div className="mt-3 flex items-center gap-2">
        {!isReady && !isDownloading && (
          <button
            type="button"
            onClick={handleDownload}
            className="flex items-center gap-1.5 rounded-xl bg-cyan-600 px-4 py-2 text-[12px] font-semibold text-white shadow-sm transition-all hover:bg-cyan-700"
          >
            <Download className="h-4 w-4" />
            下载模型
          </button>
        )}
        {isDownloading && (
          <button
            type="button"
            disabled
            className="flex items-center gap-1.5 rounded-xl bg-cyan-600/40 px-4 py-2 text-[12px] font-semibold text-white/70 cursor-not-allowed"
          >
            <Loader2 className="h-4 w-4 animate-spin" />
            下载中…
          </button>
        )}
        {isReady && (
          <button
            type="button"
            onClick={() => void loadStatus()}
            className="flex items-center gap-1.5 rounded-xl border border-black/[0.09] bg-black/[0.025] px-3 py-2 text-[11px] font-medium text-black/50 transition-colors hover:bg-black/[0.05] hover:text-black/70"
          >
            <RefreshCw className="h-3.5 w-3.5" />
            刷新状态
          </button>
        )}
      </div>

      {/* Expanded details */}
      {expanded && status && (
        <div className="mt-4 border-t border-black/[0.06] pt-3">
          <FileList label="TTS 模型" files={status.tts_files} />
          <FileList label="Audio Tokenizer" files={status.tokenizer_files} />
          <div className="mt-3 text-[10px] text-muted-foreground">
            <p>
              <span className="font-semibold text-black/30">总计:</span>{' '}
              {formatBytes(status.total_bytes)} ·{' '}
              {formatBytes(status.missing_bytes)} 待下载
            </p>
            <p className="mt-1">
              <span className="font-semibold text-black/30">来源:</span>{' '}
              HuggingFace · OpenMOSS-Team/MOSS-TTS-Nano-100M-ONNX + MOSS-Audio-Tokenizer-Nano-ONNX
            </p>
          </div>
        </div>
      )}
    </SettingsSurface>
  )
}
