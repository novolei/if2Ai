/**
 * STT 配置页 —— 仅 SenseVoice (OpenFlow)。
 *
 * Apr 2026 简化为单 backend：whisper.cpp 本地 + Groq Whisper API 都已移除。
 * 用户只需要管理 SenseVoice 模型的下载/状态即可。
 */

import { useCallback, useEffect, useRef, useState } from 'react'
import { toast } from 'sonner'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { CheckCircle, AlertCircle, Loader2, Download, Sparkles } from 'lucide-react'
import { SettingsSurface } from '../components/SettingsSurface'
import {
  sttModelStatus,
  sttDownloadOpenflowModel,
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

export function SttConfigPage() {
  const [status, setStatus] = useState<SttModelStatusResponse | null>(null)
  const [downloading, setDownloading] = useState(false)
  const [progress, setProgress] = useState<OpenFlowDownloadProgress | null>(null)
  const unlistenRef = useRef<UnlistenFn | null>(null)

  const refresh = useCallback(async () => {
    try {
      setStatus(await sttModelStatus())
    } catch (e) {
      console.error('refresh failed', e)
    }
  }, [])

  useEffect(() => {
    void refresh()
  }, [refresh])

  // 订阅下载进度事件
  useEffect(() => {
    let mounted = true
    void listen<OpenFlowDownloadProgress>('stt:openflow-download-progress', (e) => {
      if (mounted) setProgress(e.payload)
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

  const handleDownload = useCallback(async () => {
    if (downloading) return
    setDownloading(true)
    setProgress(null)
    toast.info('开始下载 SenseVoice 模型', {
      description: '约 230MB，根据网速 1-5 分钟。下载源会自动 fallback hf-mirror。',
    })
    try {
      const path = await sttDownloadOpenflowModel({ preset: 'quantized' })
      toast.success('SenseVoice 下载完成', {
        description: path.split('/').slice(-2).join('/'),
        duration: 4000,
      })
      void broadcastChange('cross:stt-settings-changed', { openflow_ready: true })
      await refresh()
    } catch (e) {
      toast.error('下载失败', { description: String(e) })
    } finally {
      setDownloading(false)
      setProgress(null)
    }
  }, [downloading, refresh])

  return (
    <div className="flex flex-col gap-3">
      {/* ── Hero / 简介 ── */}
      <SettingsSurface className="px-5 py-4">
        <SectionLabel>STT 语音输入</SectionLabel>
        <div className="flex items-start gap-3">
          <div
            className={`flex size-10 items-center justify-center rounded-xl ${
              status?.openflow_ready ? 'bg-jade/12' : 'bg-amber-500/10'
            }`}
          >
            <Sparkles
              className={`size-5 ${status?.openflow_ready ? 'text-jade' : 'text-amber-600'}`}
            />
          </div>
          <div className="flex-1">
            <div className="text-[12.5px] font-semibold">SenseVoice 本地（open-flow）</div>
            <p className="mt-0.5 text-[11px] leading-4 text-muted-foreground">
              FunASR 出品的 SenseVoice-Small 量化模型，ONNX CPU 推理。
              中/英/粤/日/韩五语支持，自带标点符号。完全离线，无网络依赖。
            </p>
            <div className="mt-1 text-[10px]">
              {status?.openflow_ready ? (
                <span className="text-jade">✓ 模型已就绪</span>
              ) : (
                <span className="text-amber-600">⚠ 模型未下载（点击下方按钮一键下载）</span>
              )}
            </div>
          </div>
        </div>
      </SettingsSurface>

      {/* ── 模型管理 ── */}
      <SettingsSurface className="px-5 py-4">
        <div className="mb-2 flex items-center justify-between">
          <SectionLabel>SenseVoice 模型</SectionLabel>
          {status?.openflow_model_dir && (
            <span className="font-mono text-[9.5px] text-black/30">
              {status.openflow_model_dir.replace(/.*\/Users\/[^/]+/, '~')}
            </span>
          )}
        </div>
        <div
          className={`flex items-center gap-3 rounded-xl border px-3 py-2 ${
            status?.openflow_ready
              ? 'border-jade/40 bg-jade/5'
              : 'border-black/[0.07] bg-white'
          }`}
        >
          <div className="min-w-0 flex-1">
            <div className="text-[11.5px] font-semibold">
              SenseVoice-Small Quantized
              <span className="ml-1.5 text-[9.5px] font-normal text-muted-foreground">~230 MB</span>
            </div>
            <div className="text-[10px] text-muted-foreground">
              FunASR 出品 · 中/英/粤/日/韩 · ONNX CPU 推理 · 自带标点
            </div>
            {downloading && progress && (
              <div className="mt-1 flex items-center gap-2 text-[10px]">
                <div className="h-1.5 flex-1 overflow-hidden rounded-full bg-black/[0.08]">
                  <div
                    className="h-full rounded-full bg-jade transition-all"
                    style={{
                      width: progress.percent >= 0 ? `${progress.percent}%` : '40%',
                    }}
                  />
                </div>
                <span className="font-mono text-black/55">
                  {progress.file} · {(progress.downloaded / 1_048_576).toFixed(1)} MB
                  {progress.percent >= 0 ? ` (${progress.percent}%)` : ''}
                </span>
              </div>
            )}
          </div>
          {status?.openflow_ready ? (
            <span className="flex items-center gap-1 text-[10px] text-jade">
              <CheckCircle className="size-3" /> 已安装
            </span>
          ) : (
            <button
              type="button"
              onClick={() => void handleDownload()}
              disabled={downloading}
              className="flex items-center gap-1 rounded-lg bg-foreground/85 px-2.5 py-1 text-[10.5px] font-semibold text-white hover:bg-foreground disabled:opacity-50"
            >
              {downloading ? (
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

      {/* ── 提示 ── */}
      <SettingsSurface className="px-5 py-3">
        <div className="flex items-start gap-2 text-[10.5px] text-muted-foreground">
          <AlertCircle className="mt-0.5 size-3.5 shrink-0 text-amber-600" />
          <div>
            使用语音输入：聊天输入框右侧的 🎤 麦克按钮 → 点击开始 / 再点结束 →
            转写完成自动填入输入框。首次使用会弹 macOS 麦克风授权对话框。
          </div>
        </div>
      </SettingsSurface>
    </div>
  )
}
