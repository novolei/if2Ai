import { useCallback, useEffect, useState } from 'react'
import { toast } from 'sonner'
import { Brain, Download, AlertTriangle, Check } from 'lucide-react'
import { SettingsSurface } from '../components/SettingsSurface'
import {
  getMemoryConfig,
  setMemoryConfig,
  exportTrajectories,
  type MemoryConfigInput,
} from '@/lib/tauri'

interface SlotConfig {
  label: string
  description: string
  value: number
  color: string
}

const DEFAULT_SLOTS: SlotConfig[] = [
  { label: '系统提示', description: 'System Prompt', value: 10, color: 'text-violet-600' },
  { label: '情景记忆', description: 'Episodic', value: 20, color: 'text-blue-600' },
  { label: '语义记忆', description: 'Semantic', value: 30, color: 'text-emerald-600' },
  { label: '工作记忆', description: 'Working', value: 40, color: 'text-amber-600' },
]

export function MemorySettingsPage() {
  const [totalTokens, setTotalTokens] = useState(4000)
  const [slots, setSlots] = useState<SlotConfig[]>(DEFAULT_SLOTS)
  const [trajectoryCount, setTrajectoryCount] = useState(0)
  const [loading, setLoading] = useState(true)
  const [saving, setSaving] = useState(false)
  const [exporting, setExporting] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const totalPercentage = slots.reduce((sum, s) => sum + s.value, 0)
  const isValid = totalPercentage === 100

  const loadConfig = async () => {
    setLoading(true)
    setError(null)
    try {
      const cfg = await getMemoryConfig()
      setTotalTokens(cfg.total_tokens)
      setTrajectoryCount(cfg.trajectory_count)
      setSlots([
        { ...DEFAULT_SLOTS[0], value: cfg.system_pct },
        { ...DEFAULT_SLOTS[1], value: cfg.episodic_pct },
        { ...DEFAULT_SLOTS[2], value: cfg.semantic_pct },
        { ...DEFAULT_SLOTS[3], value: cfg.working_pct },
      ])
    } catch (e) {
      setError(String(e))
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    void loadConfig()
  }, [])

  const updateSlot = useCallback((index: number, value: number) => {
    setSlots((prev) => {
      const next = [...prev]
      next[index] = { ...next[index], value }
      return next
    })
  }, [])

  const handleSave = async () => {
    if (!isValid) {
      toast.warning('配置无效', { description: '百分比总和必须为 100%' })
      return
    }
    setSaving(true)
    setError(null)
    try {
      const config: MemoryConfigInput = {
        total_tokens: totalTokens,
        system_pct: slots[0].value,
        episodic_pct: slots[1].value,
        semantic_pct: slots[2].value,
        working_pct: slots[3].value,
      }
      await setMemoryConfig(config)
      toast.success('配置已保存')
    } catch (e) {
      setError(String(e))
      toast.error('保存失败', { description: String(e) })
    } finally {
      setSaving(false)
    }
  }

  const handleExport = async () => {
    setError(null)
    setExporting(true)
    try {
      const msg = await exportTrajectories()
      toast.success('轨迹导出成功', { description: msg })
    } catch (e) {
      setError(String(e))
      toast.error('导出失败', { description: String(e) })
    } finally {
      setExporting(false)
    }
  }

  if (loading) {
    return (
      <div className="flex items-center justify-center py-12 text-sm text-muted-foreground">
        加载中...
      </div>
    )
  }

  return (
    <div className="flex flex-col gap-6">
      {/* Token Budget */}
      <SettingsSurface>
        <div className="flex items-center gap-3 border-b border-border/50 bg-black/[0.02] px-5 py-4">
          <Brain className="h-5 w-5 text-violet-600" />
          <div>
            <div className="text-sm font-semibold">Token 预算</div>
            <div className="text-xs text-muted-foreground">
              配置 Agent 上下文的 Token 分配比例
            </div>
          </div>
        </div>

        <div className="space-y-5 px-5 py-5">
          {/* Total tokens */}
          <div className="space-y-2">
            <label className="text-xs font-medium text-muted-foreground">
              总 Token 数
            </label>
            <input
              type="number"
              value={totalTokens}
              min={1000}
              max={128000}
              step={500}
              onChange={(e) => setTotalTokens(Math.max(1000, Number(e.target.value) || 1000))}
              className="h-10 w-40 rounded-lg border border-border/70 bg-white/60 px-3 text-sm font-mono outline-none focus:border-violet-400 focus:ring-1 focus:ring-violet-400"
            />
          </div>

          {/* Percentage sliders */}
          {slots.map((slot, idx) => (
            <div key={slot.description} className="space-y-2">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <span className={`text-xs font-semibold ${slot.color}`}>{slot.label}</span>
                  <span className="text-xs text-muted-foreground">{slot.description}</span>
                </div>
                <span className={`text-sm font-bold tabular-nums ${slot.color}`}>
                  {slot.value}%
                </span>
              </div>
              <input
                type="range"
                min={0}
                max={100}
                value={slot.value}
                onChange={(e) => updateSlot(idx, Number(e.target.value))}
                className="w-full accent-violet-500"
              />
            </div>
          ))}

          {/* Total percentage indicator */}
          <div className="flex items-center justify-between rounded-lg bg-black/[0.03] px-4 py-2">
            <span className="text-xs font-medium text-muted-foreground">
              百分比合计
            </span>
            <div className="flex items-center gap-1.5">
              {!isValid && (
                <AlertTriangle className="h-3.5 w-3.5 text-amber-500" />
              )}
              {isValid && (
                <Check className="h-3.5 w-3.5 text-emerald-500" />
              )}
              <span
                className={`text-sm font-bold tabular-nums ${
                  isValid ? 'text-emerald-600' : 'text-amber-600'
                }`}
              >
                {totalPercentage}%
              </span>
            </div>
          </div>

          {/* Save button */}
          <button
            type="button"
            disabled={saving || !isValid}
            onClick={handleSave}
            className="h-9 rounded-lg bg-violet-600 px-4 text-sm font-medium text-white transition hover:bg-violet-700 disabled:cursor-not-allowed disabled:opacity-50"
          >
            {saving ? '保存中...' : '保存配置'}
          </button>
        </div>
      </SettingsSurface>

      {/* Trajectory Export */}
      <SettingsSurface>
        <div className="flex items-center gap-3 border-b border-border/50 bg-black/[0.02] px-5 py-4">
          <Download className="h-5 w-5 text-emerald-600" />
          <div className="flex-1">
            <div className="text-sm font-semibold">轨迹导出</div>
            <div className="text-xs text-muted-foreground">
              导出 Agent 运行轨迹记录，当前有 {trajectoryCount} 条轨迹
            </div>
          </div>
        </div>

        <div className="px-5 py-5">
          <button
            type="button"
            disabled={exporting}
            onClick={handleExport}
            className="flex h-9 items-center gap-2 rounded-lg border border-border/70 bg-white/60 px-4 text-sm font-medium transition hover:bg-white/80 disabled:cursor-not-allowed disabled:opacity-50"
          >
            {exporting ? (
              <>
                <Download className="h-4 w-4 animate-spin" />
                导出中...
              </>
            ) : (
              <>
                <Download className="h-4 w-4" />
                导出轨迹
              </>
            )}
          </button>
        </div>
      </SettingsSurface>

      {/* Error message */}
      {error && (
        <div className="flex items-center gap-2 rounded-lg border border-red-200 bg-red-50 px-4 py-2.5 text-xs text-red-700">
          <AlertTriangle className="h-3.5 w-3.5" />
          {error}
        </div>
      )}
    </div>
  )
}
