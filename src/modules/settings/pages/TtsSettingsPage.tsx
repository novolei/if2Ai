/**
 * TtsSettingsPage — user-facing entry point for text-to-speech runtime
 * parameters.  Persisted to `~/.if2ai/tts.toml` via the Rust
 * `TtsSettings` struct (`get_tts_settings` / `set_tts_settings`).
 *
 * Three sections, ordered by user-facing importance:
 *
 * 1. **Speed** — `playback_rate` slider 0.5×-2×, applied frontend-side
 *    via `AudioBufferSourceNode.playbackRate`.  This is the only true
 *    speed control because the underlying MOSS-TTS-Nano model does
 *    not have a speed-factor knob.
 *
 * 2. **Quality** — three-way preset (Natural / Balanced / Precise) that
 *    maps server-side to a tuned `audio_temperature` / `top_p` / `top_k`
 *    triple.  Saves the user from having to understand sampler theory.
 *
 * 3. **Advanced** — collapsible: `max_new_frames`, repetition penalty,
 *    fixed seed, robust normaliser toggle.  For power users who want
 *    full control without editing tts.toml by hand.
 *
 * Changes are persisted on every commit (slider release, dropdown
 * change, input blur) and a toast confirms the save.  The voice bridge
 * + per-message voice button + Web Audio player all read these
 * settings on next playback, so the user sees the effect immediately
 * the next time they ask the agent to speak.
 */
import { useCallback, useEffect, useState } from 'react'
import { toast } from 'sonner'
import { ChevronDown, ChevronUp, Gauge } from 'lucide-react'
import { SettingsSurface } from '../components/SettingsSurface'
import { SettingsRow } from '../components/SettingsRow'
import { CompactInput } from '../components/CompactInput'
import {
  getTtsSettings,
  setTtsSettings,
  TTS_DEFAULT_SETTINGS,
  type TtsQualityPreset,
  type TtsSettings,
} from '@/lib/tauri'
import { broadcastChange } from '@/lib/crossWindowSync'
import { cn } from '@/lib/utils'

// ── Helpers ──────────────────────────────────────────────────────────────────

const QUALITY_LABEL: Record<TtsQualityPreset, string> = {
  natural: '自然',
  balanced: '平衡 (默认)',
  precise: '精确',
}

const QUALITY_HINT: Record<TtsQualityPreset, string> = {
  natural:
    '采样温度略高，韵律更接近真人；偶尔出现意料之外的停顿。适合对话/闲聊场景。',
  balanced:
    'MOSS-TTS-Nano 上游默认值，自然度与稳定性的平衡。绝大多数场景推荐。',
  precise:
    '低温度 + 紧 top_p，输出更可预测、更"机械感"，但发音错误更少。适合朗读/正式播报。',
}

function formatRate(rate: number): string {
  return `${rate.toFixed(2)}×`
}

function rateBucket(rate: number): string {
  if (rate < 0.85) return '慢速'
  if (rate < 1.15) return '正常'
  if (rate < 1.5) return '快速'
  return '极快'
}

// ── Component ────────────────────────────────────────────────────────────────

export function TtsSettingsPage() {
  const [settings, setSettings] = useState<TtsSettings | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [advancedOpen, setAdvancedOpen] = useState(false)

  const refresh = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      setSettings(await getTtsSettings())
    } catch (err) {
      setError(String(err))
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    void refresh()
  }, [refresh])

  const persist = async (next: TtsSettings, message?: string) => {
    try {
      await setTtsSettings(next)
      setSettings(next)
      // Notify the main window's agent-voice bridge so playback rate +
      // generation params take effect on the next utterance without
      // requiring a Settings window close.
      try {
        await broadcastChange('cross:tts-settings-changed', {})
      } catch {
        /* non-fatal */
      }
      if (message) toast.success(message)
    } catch (err) {
      toast.error('保存失败', { description: String(err) })
    }
  }

  const handleRateChange = (value: number) => {
    if (!settings) return
    void persist({ ...settings, playback_rate: value })
  }

  const handleQualityChange = (next: TtsQualityPreset) => {
    if (!settings) return
    void persist({ ...settings, quality: next }, `已切换到「${QUALITY_LABEL[next]}」预设`)
  }

  const handleNumberBlur = (
    field: 'max_new_frames' | 'audio_repetition_penalty',
    raw: string,
    min: number,
    max: number,
  ) => {
    if (!settings) return
    const parsed = Number(raw)
    if (!Number.isFinite(parsed)) {
      toast.error('请输入有效数字')
      return
    }
    const clamped = Math.min(Math.max(parsed, min), max)
    void persist({ ...settings, [field]: clamped })
  }

  const handleSeedChange = (raw: string) => {
    if (!settings) return
    if (raw.trim() === '') {
      void persist({ ...settings, seed: null })
      return
    }
    const parsed = Number(raw)
    if (!Number.isInteger(parsed) || parsed < 0) {
      toast.error('seed 必须是非负整数')
      return
    }
    void persist({ ...settings, seed: parsed })
  }

  const handleResetDefaults = () => {
    if (!window.confirm('恢复到 MOSS-TTS-Nano 默认值？此操作不可撤销。')) return
    void persist({ ...TTS_DEFAULT_SETTINGS }, '已恢复默认 TTS 设置')
  }

  if (loading || !settings) {
    return (
      <div className="space-y-3 p-5">
        <SettingsSurface>
          <div className="px-5 py-8 text-center text-[12px] text-muted-foreground">
            {error ? `加载失败：${error}` : '加载 TTS 设置…'}
          </div>
        </SettingsSurface>
      </div>
    )
  }

  return (
    <div className="space-y-5 p-5">
      {/* ── Speed ── */}
      <SettingsSurface>
        <div className="border-b border-black/[0.06] px-5 py-3.5">
          <div className="flex items-center gap-2">
            <Gauge className="h-4 w-4 text-jade" />
            <div>
              <div className="text-[14px] font-semibold tracking-tight">语速</div>
              <div className="mt-0.5 text-[11.5px] text-muted-foreground">
                通过 Web Audio API 调整播放倍率，对所有 TTS 调用生效。模型本身不支持
                语速参数。
              </div>
            </div>
          </div>
        </div>
        <div className="space-y-3 px-5 py-4">
          <div className="flex items-center justify-between">
            <div className="text-[12px] text-muted-foreground">
              当前：
              <span className="ml-1 font-mono text-foreground">
                {formatRate(settings.playback_rate)}
              </span>
              <span className="ml-2 rounded-full bg-black/[0.05] px-2 py-px text-[10.5px]">
                {rateBucket(settings.playback_rate)}
              </span>
            </div>
            <button
              type="button"
              onClick={() => handleRateChange(1.0)}
              disabled={settings.playback_rate === 1.0}
              className={cn(
                'rounded-lg border border-black/[0.08] bg-white px-2.5 py-1 text-[11px] font-medium',
                'transition-colors hover:bg-black/[0.03] disabled:opacity-40',
              )}
            >
              重置到 1×
            </button>
          </div>
          <input
            type="range"
            min={0.5}
            max={2.0}
            step={0.05}
            value={settings.playback_rate}
            onChange={(e) => handleRateChange(Number(e.target.value))}
            className="w-full accent-jade"
          />
          <div className="flex justify-between text-[10.5px] text-muted-foreground">
            <span>0.5×</span>
            <span>1×</span>
            <span>2×</span>
          </div>
          <div className="rounded-lg border border-black/[0.06] bg-black/[0.02] px-3 py-2 text-[11.5px] text-muted-foreground">
            提示：±25% 范围内（0.75-1.25）几乎听不到音质损失。
            更高/更低倍率会出现轻微的音高漂移（chipmunk 效果），属于
            <code className="mx-1 rounded bg-black/[0.05] px-1 text-[11px] font-mono">
              AudioBufferSourceNode
            </code>
            的天然行为。
          </div>
        </div>
      </SettingsSurface>

      {/* ── Quality ── */}
      <SettingsSurface>
        <div className="border-b border-black/[0.06] px-5 py-3.5">
          <div className="text-[14px] font-semibold tracking-tight">音质 (采样预设)</div>
          <div className="mt-0.5 text-[11.5px] text-muted-foreground">
            一键切换 audio_temperature / top_p / top_k 组合，无需理解采样理论。
          </div>
        </div>
        <div className="space-y-2 px-4 py-3">
          {(['natural', 'balanced', 'precise'] as TtsQualityPreset[]).map((preset) => {
            const active = settings.quality === preset
            return (
              <label
                key={preset}
                className={cn(
                  'flex cursor-pointer items-start gap-3 rounded-xl border px-3.5 py-2.5 transition-colors',
                  active
                    ? 'border-jade/40 bg-jade/[0.06]'
                    : 'border-black/[0.06] bg-white hover:bg-black/[0.02]',
                )}
              >
                <input
                  type="radio"
                  name="tts-quality"
                  className="mt-[3px] accent-jade"
                  checked={active}
                  onChange={() => handleQualityChange(preset)}
                />
                <div className="min-w-0 flex-1">
                  <div className="text-[12.5px] font-medium tracking-tight">
                    {QUALITY_LABEL[preset]}
                  </div>
                  <div className="mt-0.5 text-[11px] leading-4 text-muted-foreground">
                    {QUALITY_HINT[preset]}
                  </div>
                </div>
              </label>
            )
          })}
        </div>
      </SettingsSurface>

      {/* ── Advanced ── */}
      <SettingsSurface>
        <button
          type="button"
          onClick={() => setAdvancedOpen((v) => !v)}
          className="flex w-full items-center justify-between border-b border-black/[0.06] px-5 py-3.5 text-left"
        >
          <div>
            <div className="text-[14px] font-semibold tracking-tight">高级参数</div>
            <div className="mt-0.5 text-[11.5px] text-muted-foreground">
              直接暴露 MOSS-TTS-Nano 的核心生成参数；编辑前请阅读说明。
            </div>
          </div>
          {advancedOpen ? (
            <ChevronUp className="h-4 w-4 text-muted-foreground" />
          ) : (
            <ChevronDown className="h-4 w-4 text-muted-foreground" />
          )}
        </button>
        {advancedOpen ? (
          <div className="space-y-1 px-4 py-2">
            <SettingsRow
              inline
              title="最大生成帧数 (max_new_frames)"
              description="单次合成的音频帧上限。降低 → 首字延迟更短但可能截断长句；升高 → 长句完整但首字慢。范围 64-1500，默认 375。"
            >
              <CompactInput
                type="number"
                min={64}
                max={1500}
                step={1}
                defaultValue={settings.max_new_frames}
                onBlur={(e) => handleNumberBlur('max_new_frames', e.target.value, 64, 1500)}
                style={{ width: 120 }}
              />
            </SettingsRow>
            <SettingsRow
              inline
              title="重复惩罚 (audio_repetition_penalty)"
              description="抑制连续重复的音素。听到「the the the…」类卡死循环时调高到 1.4-1.6；默认 1.2。"
            >
              <CompactInput
                type="number"
                min={1.0}
                max={2.0}
                step={0.05}
                defaultValue={settings.audio_repetition_penalty}
                onBlur={(e) =>
                  handleNumberBlur('audio_repetition_penalty', e.target.value, 1.0, 2.0)
                }
                style={{ width: 120 }}
              />
            </SettingsRow>
            <SettingsRow
              inline
              title="固定随机种子 (seed)"
              description="同一段文本 + 同一 seed 始终产生相同音频，方便对照测试。留空 = 每次随机。"
            >
              <CompactInput
                type="number"
                min={0}
                step={1}
                placeholder="(随机)"
                defaultValue={settings.seed ?? ''}
                onBlur={(e) => handleSeedChange(e.target.value)}
                style={{ width: 120 }}
              />
            </SettingsRow>
            <SettingsRow
              inline
              title="文本规范化 (enable_robust_normalization)"
              description="开启后会把数字、日期、单位等转写成易读形式（'2024' → '二零二四'）。关闭可让模型直接读字符串。"
            >
              <button
                type="button"
                onClick={() =>
                  void persist({
                    ...settings,
                    enable_robust_normalization: !settings.enable_robust_normalization,
                  })
                }
                className={cn(
                  'inline-flex h-6 w-11 items-center rounded-full transition-colors',
                  settings.enable_robust_normalization ? 'bg-jade/80' : 'bg-black/[0.15]',
                )}
              >
                <span
                  className={cn(
                    'inline-block h-5 w-5 transform rounded-full bg-white shadow transition-transform',
                    settings.enable_robust_normalization ? 'translate-x-[22px]' : 'translate-x-0.5',
                  )}
                />
              </button>
            </SettingsRow>
            <div className="mt-3 flex justify-end">
              <button
                type="button"
                onClick={handleResetDefaults}
                className="rounded-lg border border-red-300/40 bg-red-50 px-3 py-1.5 text-[11.5px] font-medium text-red-700 transition-colors hover:bg-red-100"
              >
                恢复默认
              </button>
            </div>
          </div>
        ) : null}
      </SettingsSurface>
    </div>
  )
}
