/**
 * TtsProfilesPage — manage named TTS recipes (voice + settings + text
 * post-processing).  See `modules/tts/profile.rs` for the data model.
 *
 * Layout:
 *
 * ```
 * ┌──────────────────────────────────────────────────────────────┐
 * │ 当前 Profile  [▼ 默认对话]              [+ 新建] [复制当前]      │
 * ├───────────────────────────────────────┬──────────────────────┤
 * │ Profile 列表                          │ 编辑面板               │
 * │  ● 默认对话                  [builtin] │ 名称  [        ]      │
 * │  ○ 温柔客服                  [builtin] │ 描述  [        ]      │
 * │  ○ 冷静播报                  [builtin] │ Voice [▼ zh_1     ]   │
 * │  ○ 我的客服 (clone)          [user]    │ 语速  [——●——]  1.0×   │
 * │                                       │ 音质  [Natural ▼]      │
 * │                                       │ ── 文本润色 ──         │
 * │                                       │ ☑ 句号软化（温柔）      │
 * │                                       │ ☐ 句尾省略号（沉浸）    │
 * │                                       │ ── 高级 ──             │
 * │                                       │ max_new_frames [375]   │
 * │                                       │ rep_penalty   [1.20]   │
 * │                                       │ seed          [   ]    │
 * │                                       │ [保存]  [删除]           │
 * └───────────────────────────────────────┴──────────────────────┘
 * ```
 *
 * Builtin profiles: name/voice/settings *can* be edited (the changes
 * persist), but the profile itself can never be deleted — the [删除]
 * button is replaced with [复制为我的副本].
 */
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { toast } from 'sonner'
import { Plus, Copy, Trash2, Check, Star, ChevronDown, ChevronUp } from 'lucide-react'
import { SettingsSurface } from '../components/SettingsSurface'
import { SettingsRow } from '../components/SettingsRow'
import { CompactInput } from '../components/CompactInput'
import {
  listTtsProfiles,
  saveTtsProfile,
  deleteTtsProfile,
  setDefaultTtsProfile,
  ttsListVoiceAssets,
  type TtsProfile,
  type TtsQualityPreset,
  type TtsVoiceAsset,
} from '@/lib/tauri'
import { broadcastChange } from '@/lib/crossWindowSync'
import { cn } from '@/lib/utils'

const QUALITY_LABEL: Record<TtsQualityPreset, string> = {
  natural: '自然',
  balanced: '平衡',
  precise: '精确',
}

function genUserId(): string {
  if (typeof crypto !== 'undefined' && 'randomUUID' in crypto) {
    return `user-${crypto.randomUUID()}`
  }
  return `user-${Date.now()}-${Math.floor(Math.random() * 1000000)}`
}

function emptyDraft(): TtsProfile {
  return {
    id: genUserId(),
    name: '我的 Profile',
    description: '',
    voice_id: '',
    playback_rate: 1.0,
    quality: 'natural',
    max_new_frames: 375,
    audio_repetition_penalty: 1.2,
    seed: null,
    enable_robust_normalization: true,
    postprocess: { soften_punctuation: false, add_trailing_dots: false },
    is_builtin: false,
  }
}

export function TtsProfilesPage() {
  const [book, setBook] = useState<{ defaultId: string; profiles: TtsProfile[] } | null>(null)
  const [voices, setVoices] = useState<TtsVoiceAsset[]>([])
  const [activeId, setActiveId] = useState<string | null>(null)
  const [draft, setDraft] = useState<TtsProfile | null>(null)
  const [advancedOpen, setAdvancedOpen] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const dirtyRef = useRef(false)

  const refresh = useCallback(async () => {
    try {
      const [b, v] = await Promise.all([listTtsProfiles(), ttsListVoiceAssets()])
      setBook({ defaultId: b.default_profile_id, profiles: b.profiles })
      setVoices(v)
      setError(null)
      // First load: select default profile.
      if (!activeId) {
        const target = b.profiles.find((p) => p.id === b.default_profile_id) ?? b.profiles[0]
        if (target) {
          setActiveId(target.id)
          setDraft({ ...target })
          dirtyRef.current = false
        }
      }
    } catch (err) {
      setError(String(err))
    }
  }, [activeId])

  useEffect(() => {
    void refresh()
  }, [refresh])

  const selectProfile = (p: TtsProfile) => {
    if (dirtyRef.current && !window.confirm('当前编辑未保存，切换将丢弃改动，继续？')) return
    setActiveId(p.id)
    setDraft({ ...p })
    dirtyRef.current = false
  }

  const updateDraft = <K extends keyof TtsProfile>(key: K, value: TtsProfile[K]) => {
    setDraft((d) => {
      if (!d) return d
      dirtyRef.current = true
      return { ...d, [key]: value }
    })
  }
  const updatePostprocess = (key: keyof TtsProfile['postprocess'], value: boolean) => {
    setDraft((d) => {
      if (!d) return d
      dirtyRef.current = true
      return { ...d, postprocess: { ...d.postprocess, [key]: value } }
    })
  }

  const handleSave = async () => {
    if (!draft) return
    try {
      const saved = await saveTtsProfile(draft)
      toast.success(`已保存「${saved.name}」`)
      await broadcastChange('cross:tts-profiles-changed', { id: saved.id })
      dirtyRef.current = false
      await refresh()
    } catch (err) {
      toast.error('保存失败', { description: String(err) })
    }
  }

  const handleDelete = async () => {
    if (!draft || draft.is_builtin) return
    if (!window.confirm(`删除「${draft.name}」？此操作不可撤销。`)) return
    try {
      await deleteTtsProfile(draft.id)
      toast.success('已删除')
      await broadcastChange('cross:tts-profiles-changed', { id: null })
      setActiveId(null)
      setDraft(null)
      dirtyRef.current = false
      await refresh()
    } catch (err) {
      toast.error('删除失败', { description: String(err) })
    }
  }

  const handleDuplicate = async () => {
    if (!draft) return
    const dup: TtsProfile = {
      ...draft,
      id: genUserId(),
      name: `${draft.name} (副本)`,
      is_builtin: false,
    }
    try {
      const saved = await saveTtsProfile(dup)
      toast.success(`已复制为「${saved.name}」`)
      await broadcastChange('cross:tts-profiles-changed', { id: saved.id })
      await refresh()
      setActiveId(saved.id)
      setDraft({ ...saved })
      dirtyRef.current = false
    } catch (err) {
      toast.error('复制失败', { description: String(err) })
    }
  }

  const handleNew = async () => {
    if (dirtyRef.current && !window.confirm('当前编辑未保存，新建将丢弃改动，继续？')) return
    const fresh = emptyDraft()
    if (voices[0]) fresh.voice_id = voices[0].id
    setActiveId(fresh.id)
    setDraft(fresh)
    dirtyRef.current = true
  }

  const handleSetDefault = async () => {
    if (!draft) return
    try {
      await setDefaultTtsProfile(draft.id)
      toast.success(`已将「${draft.name}」设为默认`)
      await broadcastChange('cross:tts-profiles-changed', { id: draft.id })
      await refresh()
    } catch (err) {
      toast.error('设置默认失败', { description: String(err) })
    }
  }

  const voiceOptions = useMemo(
    () =>
      voices.map((v) => ({
        id: v.id,
        label: `${v.display_name} · ${v.kind}`,
      })),
    [voices],
  )

  if (!book) {
    return (
      <div className="space-y-3 p-5">
        <SettingsSurface>
          <div className="px-5 py-8 text-center text-[12px] text-muted-foreground">
            {error ? `加载失败：${error}` : '加载 Profiles…'}
          </div>
        </SettingsSurface>
      </div>
    )
  }

  return (
    <div className="space-y-5 p-5">
      {/* ── Header ── */}
      <SettingsSurface>
        <div className="flex items-center justify-between gap-3 px-5 py-3.5">
          <div>
            <div className="text-[14px] font-semibold tracking-tight">TTS Profiles</div>
            <div className="mt-0.5 text-[11.5px] text-muted-foreground">
              一键切换"音色 + 语速 + 采样 + 文本润色"组合。聊天侧仅显示 Profile，
              不再单独选 voice。
            </div>
          </div>
          <button
            type="button"
            onClick={() => void handleNew()}
            className="inline-flex items-center gap-1.5 rounded-lg border border-black/[0.08] bg-white px-3 py-1.5 text-[11.5px] font-medium hover:bg-black/[0.03]"
          >
            <Plus className="h-3.5 w-3.5" />
            新建 Profile
          </button>
        </div>
      </SettingsSurface>

      <div className="grid grid-cols-1 gap-4 lg:grid-cols-[280px_1fr]">
        {/* ── Profile List ── */}
        <SettingsSurface>
          <div className="px-3 py-2">
            <ul className="space-y-1">
              {book.profiles.map((p) => {
                const isActive = p.id === activeId
                const isDefault = p.id === book.defaultId
                return (
                  <li key={p.id}>
                    <button
                      type="button"
                      onClick={() => selectProfile(p)}
                      className={cn(
                        'group flex w-full items-center gap-2 rounded-lg px-2.5 py-2 text-left transition-colors',
                        isActive ? 'bg-jade/[0.08]' : 'hover:bg-black/[0.03]',
                      )}
                    >
                      <div className="flex-1 min-w-0">
                        <div className="flex items-center gap-1.5">
                          <span
                            className={cn(
                              'truncate text-[12.5px]',
                              isActive ? 'font-semibold text-foreground' : 'text-foreground/85',
                            )}
                          >
                            {p.name}
                          </span>
                          {isDefault ? (
                            <Star className="h-3 w-3 fill-jade text-jade" />
                          ) : null}
                        </div>
                        <div className="mt-0.5 truncate text-[10.5px] text-muted-foreground">
                          {p.voice_id || '(未选 voice)'} · {p.playback_rate.toFixed(2)}× ·{' '}
                          {QUALITY_LABEL[p.quality]}
                        </div>
                      </div>
                      <span
                        className={cn(
                          'rounded px-1.5 py-px text-[9.5px] font-semibold uppercase tracking-wide',
                          p.is_builtin
                            ? 'bg-amber-500/15 text-amber-700'
                            : 'bg-violet-500/15 text-violet-700',
                        )}
                      >
                        {p.is_builtin ? 'builtin' : 'user'}
                      </span>
                    </button>
                  </li>
                )
              })}
            </ul>
          </div>
        </SettingsSurface>

        {/* ── Editor ── */}
        {draft ? (
          <SettingsSurface>
            <div className="flex items-center justify-between gap-3 border-b border-black/[0.06] px-5 py-3">
              <div>
                <div className="text-[12.5px] font-semibold">编辑 Profile</div>
                <div className="text-[10.5px] text-muted-foreground">
                  {draft.is_builtin ? '内置 Profile · 可改字段，不可删除' : '我的 Profile'}
                </div>
              </div>
              <div className="flex items-center gap-1.5">
                {draft.id !== book.defaultId ? (
                  <button
                    type="button"
                    onClick={() => void handleSetDefault()}
                    className="inline-flex items-center gap-1 rounded-lg border border-black/[0.08] px-2.5 py-1 text-[10.5px] hover:bg-black/[0.03]"
                  >
                    <Star className="h-3 w-3" />
                    设为默认
                  </button>
                ) : (
                  <span className="inline-flex items-center gap-1 rounded-lg bg-jade/[0.08] px-2.5 py-1 text-[10.5px] text-jade">
                    <Check className="h-3 w-3" />
                    当前默认
                  </span>
                )}
                <button
                  type="button"
                  onClick={() => void handleDuplicate()}
                  className="inline-flex items-center gap-1 rounded-lg border border-black/[0.08] px-2.5 py-1 text-[10.5px] hover:bg-black/[0.03]"
                >
                  <Copy className="h-3 w-3" />
                  复制
                </button>
                {!draft.is_builtin ? (
                  <button
                    type="button"
                    onClick={() => void handleDelete()}
                    className="inline-flex items-center gap-1 rounded-lg border border-red-300/40 bg-red-50 px-2.5 py-1 text-[10.5px] text-red-700 hover:bg-red-100"
                  >
                    <Trash2 className="h-3 w-3" />
                    删除
                  </button>
                ) : null}
              </div>
            </div>

            <div className="space-y-1 px-4 py-2">
              <SettingsRow inline title="名称">
                <CompactInput
                  value={draft.name}
                  onChange={(e) => updateDraft('name', e.target.value)}
                  placeholder="例如：温柔客服"
                  style={{ width: 220 }}
                />
              </SettingsRow>
              <SettingsRow inline title="描述" description="可选，显示在 Profile 卡片下。">
                <CompactInput
                  value={draft.description}
                  onChange={(e) => updateDraft('description', e.target.value)}
                  placeholder="可选"
                  style={{ width: 320 }}
                />
              </SettingsRow>
              <SettingsRow
                inline
                title="参考音色 (voice)"
                description="决定音色 + 情绪基调。从已注册的 voice 资产中选择；没有的话先去设置 → TTS 测试上传。"
              >
                <select
                  value={draft.voice_id}
                  onChange={(e) => updateDraft('voice_id', e.target.value)}
                  className="rounded-lg border border-black/[0.1] bg-white px-2 py-1 text-[12px] focus:border-jade/50 focus:outline-none"
                  style={{ minWidth: 240 }}
                >
                  <option value="">(未选)</option>
                  {voiceOptions.map((v) => (
                    <option key={v.id} value={v.id}>
                      {v.label}
                    </option>
                  ))}
                </select>
              </SettingsRow>

              <div className="my-2 border-t border-black/[0.05]" />

              <SettingsRow
                inline
                title="语速"
                description="0.5×–2×；前端 WebAudio playbackRate 即时生效。"
              >
                <div className="flex items-center gap-2">
                  <input
                    type="range"
                    min={0.5}
                    max={2}
                    step={0.05}
                    value={draft.playback_rate}
                    onChange={(e) => updateDraft('playback_rate', Number(e.target.value))}
                    className="w-40 accent-jade"
                  />
                  <span className="w-12 text-right font-mono text-[11.5px]">
                    {draft.playback_rate.toFixed(2)}×
                  </span>
                </div>
              </SettingsRow>
              <SettingsRow inline title="音质预设" description="自然 / 平衡 / 精确。">
                <select
                  value={draft.quality}
                  onChange={(e) => updateDraft('quality', e.target.value as TtsQualityPreset)}
                  className="rounded-lg border border-black/[0.1] bg-white px-2 py-1 text-[12px]"
                >
                  <option value="natural">自然</option>
                  <option value="balanced">平衡</option>
                  <option value="precise">精确</option>
                </select>
              </SettingsRow>

              <div className="my-2 border-t border-black/[0.05]" />

              <div className="px-1 pb-1 pt-1 text-[10.5px] font-semibold uppercase tracking-wider text-muted-foreground">
                文本润色 (提供"假情绪")
              </div>
              <SettingsRow
                inline
                title="句号软化"
                description="把 。和 . 替换成 ，和 , — 节奏更柔，适合温柔/陪伴 profile。"
              >
                <input
                  type="checkbox"
                  checked={draft.postprocess.soften_punctuation}
                  onChange={(e) => updatePostprocess('soften_punctuation', e.target.checked)}
                  className="h-4 w-4 accent-jade"
                />
              </SettingsRow>
              <SettingsRow
                inline
                title="句尾省略号"
                description="每句末尾追加 …，让收尾不那么突兀，适合朗读/沉浸 profile。"
              >
                <input
                  type="checkbox"
                  checked={draft.postprocess.add_trailing_dots}
                  onChange={(e) => updatePostprocess('add_trailing_dots', e.target.checked)}
                  className="h-4 w-4 accent-jade"
                />
              </SettingsRow>

              <button
                type="button"
                onClick={() => setAdvancedOpen((v) => !v)}
                className="mt-2 flex w-full items-center justify-between rounded-lg px-2 py-1.5 text-[11px] font-semibold uppercase tracking-wider text-muted-foreground hover:bg-black/[0.03]"
              >
                <span>高级参数</span>
                {advancedOpen ? (
                  <ChevronUp className="h-3.5 w-3.5" />
                ) : (
                  <ChevronDown className="h-3.5 w-3.5" />
                )}
              </button>
              {advancedOpen ? (
                <>
                  <SettingsRow
                    inline
                    title="max_new_frames"
                    description="单次合成帧数上限 (64-1500)。低=首字快但可能截断；高=长句完整但慢。"
                  >
                    <CompactInput
                      type="number"
                      min={64}
                      max={1500}
                      value={draft.max_new_frames}
                      onChange={(e) =>
                        updateDraft(
                          'max_new_frames',
                          Math.min(Math.max(Number(e.target.value), 64), 1500),
                        )
                      }
                      style={{ width: 100 }}
                    />
                  </SettingsRow>
                  <SettingsRow
                    inline
                    title="audio_repetition_penalty"
                    description="抗复读。听到卡循环时调到 1.4-1.6；默认 1.2。"
                  >
                    <CompactInput
                      type="number"
                      min={1.0}
                      max={2.0}
                      step={0.05}
                      value={draft.audio_repetition_penalty}
                      onChange={(e) =>
                        updateDraft(
                          'audio_repetition_penalty',
                          Math.max(Number(e.target.value), 1.0),
                        )
                      }
                      style={{ width: 100 }}
                    />
                  </SettingsRow>
                  <SettingsRow
                    inline
                    title="seed"
                    description="固定随机种子；留空 = 随机。"
                  >
                    <CompactInput
                      type="number"
                      min={0}
                      placeholder="(随机)"
                      value={draft.seed ?? ''}
                      onChange={(e) => {
                        const raw = e.target.value
                        if (raw.trim() === '') {
                          updateDraft('seed', null)
                        } else {
                          const v = Number(raw)
                          if (Number.isInteger(v) && v >= 0) updateDraft('seed', v)
                        }
                      }}
                      style={{ width: 100 }}
                    />
                  </SettingsRow>
                  <SettingsRow
                    inline
                    title="enable_robust_normalization"
                    description="开启 = 把数字/单位转成易读形式（'2024' → '二零二四'）。"
                  >
                    <input
                      type="checkbox"
                      checked={draft.enable_robust_normalization}
                      onChange={(e) =>
                        updateDraft('enable_robust_normalization', e.target.checked)
                      }
                      className="h-4 w-4 accent-jade"
                    />
                  </SettingsRow>
                </>
              ) : null}

              <div className="mt-3 flex justify-end gap-2">
                <button
                  type="button"
                  onClick={() => void handleSave()}
                  className="rounded-lg bg-jade px-3.5 py-1.5 text-[12px] font-semibold text-white hover:bg-jade/90"
                >
                  保存
                </button>
              </div>
            </div>
          </SettingsSurface>
        ) : (
          <SettingsSurface>
            <div className="px-5 py-12 text-center text-[12px] text-muted-foreground">
              从左侧选择一个 Profile，或点 [+ 新建 Profile]。
            </div>
          </SettingsSurface>
        )}
      </div>
    </div>
  )
}
