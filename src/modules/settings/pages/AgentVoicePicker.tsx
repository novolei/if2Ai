/**
 * Phase TTS-D / P0：Agent Voice Picker
 *
 * 列出所有可用的 voice assets（builtin / bundled / user-uploaded），
 * 用户可以：
 * 1. 点 [▶ 原声] 试听原始 prompt audio（zh_1.wav 等）
 * 2. 点 [▶ 合成] 让 TTS 用此声音说一段预览文本（"你好，我是 X。"）
 * 3. 点 [设为 Agent 声音] 把它存到 localStorage / agentVoiceId
 *
 * 选中的 voice 由 `useAgentVoiceBridge` hook（聊天集成）读取。
 */

import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { toast } from 'sonner'
import { Volume2, Mic, CheckCircle, Loader2, Upload, Trash2, Pencil } from 'lucide-react'
import {
  ttsListVoiceAssets,
  ttsPreviewVoice,
  ttsVoiceAudio,
  ttsCachedVoicePreview,
  ttsUploadUserVoice,
  ttsDeleteUserVoice,
  ttsRenameUserVoice,
  ttsWarmVoicePreview,
  type TtsVoiceAsset,
} from '@/lib/tauri'
import { broadcastChange } from '@/lib/crossWindowSync'

const AGENT_VOICE_LOCAL_STORAGE_KEY = 'if2ai.tts.agentVoiceId'
const AGENT_VOICE_ENABLED_KEY = 'if2ai.tts.agentVoiceEnabled'

/** Helper：读取当前 agent voice id（其它组件 import 用）。 */
export function getAgentVoiceId(): string | null {
  try {
    return localStorage.getItem(AGENT_VOICE_LOCAL_STORAGE_KEY)
  } catch {
    return null
  }
}

/** Helper：写入当前 agent voice id。Settings 窗口和主窗口跨窗口同步走 Tauri event。 */
export function setAgentVoiceId(id: string | null): void {
  try {
    if (id == null || id === '') {
      localStorage.removeItem(AGENT_VOICE_LOCAL_STORAGE_KEY)
    } else {
      localStorage.setItem(AGENT_VOICE_LOCAL_STORAGE_KEY, id)
    }
  } catch {
    /* ignore */
  }
  // 跨窗口广播：所有窗口都会收到（包括当前），useCrossWindowChange 自动同步 React state
  void broadcastChange('cross:agent-voice-changed', { id })
}

export function getAgentVoiceEnabled(): boolean {
  try {
    return localStorage.getItem(AGENT_VOICE_ENABLED_KEY) === '1'
  } catch {
    return false
  }
}

export function setAgentVoiceEnabled(v: boolean): void {
  try {
    localStorage.setItem(AGENT_VOICE_ENABLED_KEY, v ? '1' : '0')
  } catch {
    /* ignore */
  }
  void broadcastChange('cross:agent-voice-enabled', { enabled: v })
}

interface VoiceCardProps {
  asset: TtsVoiceAsset
  isAgent: boolean
  onSetAsAgent: (id: string) => void
  onDelete?: (id: string) => Promise<void> | void
  onRename?: (id: string, newName: string) => Promise<void> | void
}

function VoiceCard({ asset, isAgent, onSetAsAgent, onDelete, onRename }: VoiceCardProps) {
  const [originalUrl, setOriginalUrl] = useState<string | null>(null)
  const [synthUrl, setSynthUrl] = useState<string | null>(null)
  const [loadingOriginal, setLoadingOriginal] = useState(false)
  const [loadingSynth, setLoadingSynth] = useState(false)
  const audioRef = useRef<HTMLAudioElement | null>(null)

  const playOriginal = useCallback(async () => {
    if (originalUrl) {
      audioRef.current?.pause()
      const a = new Audio(originalUrl)
      void a.play()
      audioRef.current = a
      return
    }
    setLoadingOriginal(true)
    try {
      const r = await ttsVoiceAudio(asset.id)
      const bytes = Uint8Array.from(atob(r.audio_base64), (c) => c.charCodeAt(0))
      const url = URL.createObjectURL(new Blob([bytes], { type: r.content_type }))
      setOriginalUrl(url)
      const a = new Audio(url)
      void a.play()
      audioRef.current = a
    } catch (e) {
      console.error('voice audio failed', e)
    } finally {
      setLoadingOriginal(false)
    }
  }, [asset.id, originalUrl])

  const playSynth = useCallback(async () => {
    if (synthUrl) {
      audioRef.current?.pause()
      const a = new Audio(synthUrl)
      void a.play()
      audioRef.current = a
      return
    }
    setLoadingSynth(true)
    try {
      // Phase TTS-E / P2: 优先从缓存 preview WAV 获取（即时），fallback 到实时合成
      const r = await ttsCachedVoicePreview(asset.id)
      const bytes = Uint8Array.from(atob(r.audio_base64), (c) => c.charCodeAt(0))
      const url = URL.createObjectURL(new Blob([bytes], { type: r.content_type }))
      setSynthUrl(url)
      const a = new Audio(url)
      void a.play()
      audioRef.current = a
    } catch {
      // fallback 到实时合成
      try {
        const r = await ttsPreviewVoice(asset.id)
        const bytes = Uint8Array.from(atob(r.audio_base64), (c) => c.charCodeAt(0))
        const url = URL.createObjectURL(new Blob([bytes], { type: 'audio/wav' }))
        setSynthUrl(url)
        const a = new Audio(url)
        void a.play()
        audioRef.current = a
      } catch (e2) {
        console.error('preview synth failed', e2)
      }
    } finally {
      setLoadingSynth(false)
    }
  }, [asset.id, synthUrl])

  useEffect(() => {
    return () => {
      if (originalUrl) URL.revokeObjectURL(originalUrl)
      if (synthUrl) URL.revokeObjectURL(synthUrl)
    }
  }, [originalUrl, synthUrl])

  const kindBadge = (
    {
      builtin: { label: 'Builtin', cls: 'bg-jade/15 text-jade' },
      bundled: { label: 'Bundled', cls: 'bg-amber-500/15 text-amber-700' },
      user: { label: 'User', cls: 'bg-violet-500/15 text-violet-700' },
    } as const
  )[asset.kind as 'builtin' | 'bundled' | 'user'] ?? { label: asset.kind, cls: 'bg-black/10 text-black/60' }

  const isUserKind = asset.kind === 'user'
  const handleRename = useCallback(() => {
    const next = prompt('重命名为：', asset.display_name)
    if (!next || next.trim() === '' || next === asset.display_name) return
    void onRename?.(asset.id, next.trim())
  }, [asset.display_name, asset.id, onRename])

  const handleDelete = useCallback(() => {
    if (!confirm(`确定删除 "${asset.display_name}" 吗？此操作不可撤销。`)) return
    void onDelete?.(asset.id)
  }, [asset.display_name, asset.id, onDelete])

  return (
    <div
      className={`flex flex-col gap-1.5 rounded-xl border px-3 py-2.5 text-[11px] transition-colors ${
        isAgent ? 'border-jade/40 bg-jade/5' : 'border-black/[0.07] bg-white hover:bg-black/[0.018]'
      }`}
    >
      <div className="flex items-center gap-2">
        <span className="flex-1 truncate font-semibold text-foreground/85">{asset.display_name}</span>
        <span className={`rounded-md px-1.5 py-0.5 text-[9px] font-semibold tracking-wide uppercase ${kindBadge.cls}`}>
          {kindBadge.label}
        </span>
        {asset.language && (
          <span className="rounded-md bg-black/[0.06] px-1.5 py-0.5 text-[9px] font-mono text-black/55">
            {asset.language.toUpperCase()}
          </span>
        )}
        {isUserKind && (
          <>
            <button
              type="button"
              onClick={handleRename}
              className="rounded p-0.5 text-black/35 hover:bg-black/5 hover:text-black/70"
              title="重命名"
            >
              <Pencil className="size-3" />
            </button>
            <button
              type="button"
              onClick={handleDelete}
              className="rounded p-0.5 text-rose-500/60 hover:bg-rose-50 hover:text-rose-600"
              title="删除（不可撤销）"
            >
              <Trash2 className="size-3" />
            </button>
          </>
        )}
      </div>
      <div className="flex items-center gap-1">
        <button
          type="button"
          disabled={!asset.is_previewable || loadingOriginal}
          onClick={() => void playOriginal()}
          className="flex flex-1 items-center justify-center gap-1 rounded-lg border border-black/[0.07] bg-white px-2 py-1 text-[10.5px] text-black/60 hover:text-black disabled:opacity-30"
          title={asset.is_previewable ? '试听原声' : '此 voice 没有原声文件'}
        >
          {loadingOriginal ? <Loader2 className="size-3 animate-spin" /> : <Volume2 className="size-3" />}
          原声
        </button>
        <button
          type="button"
          disabled={loadingSynth}
          onClick={() => void playSynth()}
          className="flex flex-1 items-center justify-center gap-1 rounded-lg border border-black/[0.07] bg-white px-2 py-1 text-[10.5px] text-black/60 hover:text-black disabled:opacity-30"
          title="用 TTS 合成一段预览"
        >
          {loadingSynth ? <Loader2 className="size-3 animate-spin" /> : <Mic className="size-3" />}
          合成
        </button>
        <button
          type="button"
          onClick={() => onSetAsAgent(asset.id)}
          disabled={isAgent}
          className={`flex items-center justify-center gap-1 rounded-lg px-2 py-1 text-[10.5px] font-semibold transition-colors ${
            isAgent
              ? 'bg-jade text-white cursor-default'
              : 'border border-black/[0.07] bg-white text-black/60 hover:text-black hover:bg-jade/8'
          }`}
        >
          {isAgent ? <CheckCircle className="size-3" /> : null}
          {isAgent ? 'Agent 声音' : '设为 Agent'}
        </button>
      </div>
    </div>
  )
}

interface AgentVoicePickerProps {
  /** 上层若需要响应"agent voice 改了"，传 callback 进来。 */
  onAgentVoiceChange?: (id: string | null) => void
}

export function AgentVoicePicker({ onAgentVoiceChange }: AgentVoicePickerProps) {
  const [assets, setAssets] = useState<TtsVoiceAsset[]>([])
  const [loading, setLoading] = useState(true)
  const [agentVoice, setAgentVoiceState] = useState<string | null>(getAgentVoiceId())
  const [enabled, setEnabledState] = useState<boolean>(getAgentVoiceEnabled())
  const [filter, setFilter] = useState<'all' | 'builtin' | 'bundled' | 'user'>('all')
  // Phase TTS-E.2：上传/拖拽
  const [uploading, setUploading] = useState(false)
  const [dragOver, setDragOver] = useState(false)
  const fileInputRef = useRef<HTMLInputElement | null>(null)

  const refreshAssets = useCallback(async () => {
    try {
      const list = await ttsListVoiceAssets()
      setAssets(list)
    } catch (e) {
      console.error('list voice assets failed', e)
    }
  }, [])

  useEffect(() => {
    let alive = true
    void (async () => {
      setLoading(true)
      try {
        const list = await ttsListVoiceAssets()
        if (alive) setAssets(list)
      } catch (e) {
        console.error('list voice assets failed', e)
      } finally {
        if (alive) setLoading(false)
      }
    })()
    return () => {
      alive = false
    }
  }, [])

  // Phase TTS-E.2：上传文件 → backend → 自动刷新
  const handleUploadFile = useCallback(async (file: File) => {
    if (uploading) return
    const ext = file.name.split('.').pop()?.toLowerCase() ?? ''
    if (!['wav', 'mp3', 'flac', 'ogg', 'm4a'].includes(ext)) {
      toast.error(`不支持的格式：.${ext}`, {
        description: '仅支持 wav / mp3 / flac / ogg / m4a',
      })
      return
    }
    if (file.size > 30 * 1024 * 1024) {
      toast.error(`文件过大 (${Math.round(file.size / 1024 / 1024)}MB)`, {
        description: '最大 30MB；推荐 5-30 秒长度的清晰人声片段',
      })
      return
    }
    setUploading(true)
    try {
      const buf = await file.arrayBuffer()
      const bytes = new Uint8Array(buf)
      const result = await ttsUploadUserVoice(file.name, bytes)
      toast.success('上传成功', {
        description: `已添加 "${result.asset.display_name}" · 正在后台预合成预览…`,
      })
      await refreshAssets()
      setFilter('user')
      // Phase TTS-E / P2：fire-and-forget 预合成 preview WAV
      void ttsWarmVoicePreview(result.asset.id).then(() => {
        toast.info(`"${result.asset.display_name}" 预览已缓存`, {
          description: '下次点击 ▶ 合成 将即时播放',
          duration: 2000,
        })
      }).catch(() => { /* silent – preview is optional */ })
    } catch (e) {
      toast.error('上传失败', { description: String(e) })
    } finally {
      setUploading(false)
    }
  }, [uploading, refreshAssets])

  const handleFileInputChange = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    const files = e.target.files
    if (!files || files.length === 0) return
    void handleUploadFile(files[0])
    e.target.value = '' // 允许同名文件重选触发 onChange
  }, [handleUploadFile])

  const handleDrop = useCallback((e: React.DragEvent) => {
    e.preventDefault()
    setDragOver(false)
    const files = e.dataTransfer?.files
    if (!files || files.length === 0) return
    void handleUploadFile(files[0])
  }, [handleUploadFile])

  const handleDeleteUserVoice = useCallback(async (id: string) => {
    try {
      await ttsDeleteUserVoice(id)
      // 如果删的是 agent voice，清掉
      if (agentVoice === id) {
        setAgentVoiceId(null)
        setAgentVoiceState(null)
      }
      toast.success('已删除')
      await refreshAssets()
    } catch (e) {
      toast.error('删除失败', { description: String(e) })
    }
  }, [agentVoice, refreshAssets])

  const handleRenameUserVoice = useCallback(async (id: string, newName: string) => {
    try {
      await ttsRenameUserVoice(id, newName)
      toast.success('已重命名')
      await refreshAssets()
    } catch (e) {
      toast.error('重命名失败', { description: String(e) })
    }
  }, [refreshAssets])

  const handleSetAgent = useCallback((id: string) => {
    setAgentVoiceId(id)
    setAgentVoiceState(id)
    // 关键 UX 修复：选 Agent 声音时自动启用"聊天中语音回复"，避免用户漏开 toggle
    if (!enabled) {
      setAgentVoiceEnabled(true)
      setEnabledState(true)
      toast.success('已启用聊天语音回复', {
        description: 'Agent 回复时会用所选声音自动朗读',
        duration: 2400,
      })
    } else {
      toast.success('Agent 声音已切换')
    }
    onAgentVoiceChange?.(id)
  }, [enabled, onAgentVoiceChange])

  const handleClear = useCallback(() => {
    setAgentVoiceId(null)
    setAgentVoiceState(null)
    onAgentVoiceChange?.(null)
  }, [onAgentVoiceChange])

  const filtered = useMemo(() => {
    if (filter === 'all') return assets
    return assets.filter((v) => v.kind === filter)
  }, [assets, filter])

  const counts = useMemo(() => ({
    all: assets.length,
    builtin: assets.filter((v) => v.kind === 'builtin').length,
    bundled: assets.filter((v) => v.kind === 'bundled').length,
    user: assets.filter((v) => v.kind === 'user').length,
  }), [assets])

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center justify-between gap-3">
        <div className="text-[10.5px] font-semibold uppercase tracking-widest text-black/35">
          Agent 语音 · {agentVoice ?? <span className="text-black/30">未选择</span>}
        </div>
        <div className="flex items-center gap-3">
          <label className="flex items-center gap-1.5 text-[10.5px] text-black/55 select-none cursor-pointer">
            <input
              type="checkbox"
              checked={enabled}
              disabled={!agentVoice}
              onChange={(e) => {
                const v = e.target.checked
                setAgentVoiceEnabled(v)
                setEnabledState(v)
              }}
              className="h-3.5 w-3.5 rounded border-black/20 text-jade focus:ring-jade/30 disabled:opacity-30"
            />
            <span>聊天中语音回复</span>
          </label>
          {agentVoice && (
            <button
              type="button"
              onClick={handleClear}
              className="text-[10px] text-black/40 hover:text-black/70"
            >
              清除
            </button>
          )}
        </div>
      </div>

      {/* Filter tabs */}
      <div className="flex gap-1 text-[10px]">
        {(['all', 'builtin', 'bundled', 'user'] as const).map((k) => (
          <button
            key={k}
            type="button"
            onClick={() => setFilter(k)}
            className={`rounded-md px-2 py-0.5 transition-colors ${
              filter === k
                ? 'bg-foreground/85 text-white'
                : 'text-black/45 hover:bg-black/[0.04]'
            }`}
          >
            {k === 'all' ? '全部' : k === 'builtin' ? 'Builtin' : k === 'bundled' ? 'Bundled' : 'User'}
            <span className="ml-1 text-[9px] opacity-60">{counts[k]}</span>
          </button>
        ))}
      </div>

      {loading ? (
        <div className="flex items-center justify-center py-8 text-[11px] text-muted-foreground">
          <Loader2 className="mr-2 size-3 animate-spin" />
          加载语音资产中...
        </div>
      ) : filtered.length === 0 ? (
        <div className="rounded-xl border border-dashed border-black/[0.12] px-4 py-6 text-center text-[11px] text-muted-foreground/60">
          没有匹配的 voice。把 .wav / .mp3 放到 <code className="font-mono">src-tauri/resources/voices/</code> 即可作为 bundled voice。
        </div>
      ) : (
        <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
          {filtered.map((asset) => (
            <VoiceCard
              key={asset.id}
              asset={asset}
              isAgent={asset.id === agentVoice}
              onSetAsAgent={handleSetAgent}
              onDelete={handleDeleteUserVoice}
              onRename={handleRenameUserVoice}
            />
          ))}
        </div>
      )}

      {/* Phase TTS-E.2：自定义上传区（拖拽 + 文件选择） */}
      <div
        onDragOver={(e) => {
          e.preventDefault()
          setDragOver(true)
        }}
        onDragLeave={() => setDragOver(false)}
        onDrop={handleDrop}
        className={`flex flex-col items-center gap-1 rounded-xl border-2 border-dashed px-4 py-4 text-[11px] transition-colors ${
          dragOver
            ? 'border-jade/60 bg-jade/8'
            : 'border-black/[0.12] bg-black/[0.018] hover:border-black/[0.22]'
        } ${uploading ? 'pointer-events-none opacity-50' : 'cursor-pointer'}`}
        onClick={() => fileInputRef.current?.click()}
      >
        <input
          ref={fileInputRef}
          type="file"
          accept=".wav,.mp3,.flac,.ogg,.m4a,audio/*"
          className="hidden"
          onChange={handleFileInputChange}
        />
        {uploading ? (
          <>
            <Loader2 className="size-4 animate-spin text-jade" />
            <span className="text-[10.5px] text-muted-foreground">上传中...</span>
          </>
        ) : (
          <>
            <Upload className="size-4 text-black/40" />
            <span className="text-[11px] font-semibold text-foreground/80">
              拖拽音频到此处 或 点击选择文件
            </span>
            <span className="text-[10px] text-muted-foreground">
              支持 wav / mp3 / flac / ogg / m4a · 推荐 5-30 秒人声 · 最大 30 MB
            </span>
          </>
        )}
      </div>
    </div>
  )
}
