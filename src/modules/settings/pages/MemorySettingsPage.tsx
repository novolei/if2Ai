import { useCallback, useEffect, useState } from 'react'
import { toast } from 'sonner'
import { Brain, Download, AlertTriangle, Check, Sliders } from 'lucide-react'
import { SettingsSurface } from '../components/SettingsSurface'
import {
  getMemoryConfig,
  setMemoryConfig,
  exportTrajectories,
  type MemoryConfigInput,
  type MemoryRecallMode,
  type MemoryPolicyEnforceMode,
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

  // Memory Control Plane V1 feature flags (FE-B / FE-Settings).
  const [controlPlaneEnabled, setControlPlaneEnabled] = useState(true)
  const [recallMode, setRecallMode] = useState<MemoryRecallMode>('hybrid')
  const [policyEnforceMode, setPolicyEnforceMode] = useState<MemoryPolicyEnforceMode>('shadow')

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
      setControlPlaneEnabled(cfg.control_plane_v1_enabled)
      setRecallMode(cfg.recall_mode)
      setPolicyEnforceMode(cfg.policy_enforce_mode)
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
        control_plane_v1_enabled: controlPlaneEnabled,
        recall_mode: recallMode,
        policy_enforce_mode: policyEnforceMode,
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
      <div className="flex items-center justify-center py-16 text-[12px] text-muted-foreground">
        加载中...
      </div>
    )
  }

  return (
    <div className="flex flex-col gap-3">
      {/* ── Token Budget ── */}
      <SettingsSurface className="px-5 py-4">
        <div className="mb-3 flex items-center gap-2.5">
          <div className="flex size-7 items-center justify-center rounded-lg bg-violet-500/[0.1]">
            <Brain className="h-3.5 w-3.5 text-violet-600" />
          </div>
          <div className="text-[10.5px] font-semibold uppercase tracking-widest text-black/30">
            Token 预算
          </div>
        </div>
        <p className="mb-4 text-[11.5px] text-muted-foreground">
          配置 Agent 上下文的 Token 分配比例
        </p>

        {/* Total tokens */}
        <div className="mb-4 flex items-center gap-3">
          <label className="text-[12px] font-medium text-foreground/70 shrink-0">总 Token 数</label>
          <input
            type="number"
            value={totalTokens}
            min={1000}
            max={128000}
            step={500}
            onChange={(e) => setTotalTokens(Math.max(1000, Number(e.target.value) || 1000))}
            className="h-8 w-36 rounded-xl border border-black/[0.09] bg-black/[0.025] px-3 font-mono text-[12px] outline-none transition-all focus:border-jade/40 focus:ring-[3px] focus:ring-jade/15"
          />
        </div>

        {/* Percentage sliders */}
        <div className="flex flex-col gap-4">
          {slots.map((slot, idx) => (
            <div key={slot.description}>
              <div className="mb-1.5 flex items-center justify-between">
                <div className="flex items-center gap-1.5">
                  <span className={`text-[12px] font-semibold ${slot.color}`}>{slot.label}</span>
                  <span className="text-[11px] text-muted-foreground">{slot.description}</span>
                </div>
                <span className={`font-mono text-[13px] font-bold tabular-nums ${slot.color}`}>
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
        </div>

        {/* Total indicator + save */}
        <div className="mt-4 flex items-center justify-between">
          <div className="flex items-center gap-2 rounded-xl border border-black/[0.06] bg-black/[0.02] px-3 py-1.5">
            {isValid ? (
              <Check className="h-3.5 w-3.5 text-emerald-500" />
            ) : (
              <AlertTriangle className="h-3.5 w-3.5 text-amber-500" />
            )}
            <span className={`font-mono text-[12px] font-bold tabular-nums ${isValid ? 'text-emerald-600' : 'text-amber-600'}`}>
              {totalPercentage}%
            </span>
            <span className="text-[11px] text-muted-foreground">合计</span>
          </div>

          <button
            type="button"
            disabled={saving || !isValid}
            onClick={handleSave}
            className="h-8 rounded-xl bg-jade px-4 text-[12px] font-semibold text-white transition-colors hover:bg-jade/90 disabled:cursor-not-allowed disabled:opacity-40"
          >
            {saving ? '保存中…' : '保存配置'}
          </button>
        </div>
      </SettingsSurface>

      {/* ── Memory Control Plane V1 feature flags ── */}
      <SettingsSurface className="px-5 py-4">
        <div className="mb-3 flex items-center gap-2.5">
          <div className="flex size-7 items-center justify-center rounded-lg bg-blue-500/[0.1]">
            <Sliders className="h-3.5 w-3.5 text-blue-600" />
          </div>
          <div className="text-[10.5px] font-semibold uppercase tracking-widest text-black/30">
            记忆控制平面 V1
          </div>
        </div>
        <p className="mb-4 text-[11.5px] text-muted-foreground">
          控制混合检索、策略执行模式与新一代记忆管线总开关
        </p>

        {/* Master kill-switch */}
        <div className="mb-4 flex items-center justify-between rounded-xl border border-black/[0.06] bg-black/[0.015] px-3 py-2.5">
          <div>
            <div className="text-[12px] font-semibold text-foreground/85">启用 Control Plane V1</div>
            <p className="mt-0.5 text-[11px] text-muted-foreground">
              关闭后所有记忆请求回退到 v0 路径（仅排错使用）
            </p>
          </div>
          <button
            type="button"
            role="switch"
            aria-checked={controlPlaneEnabled}
            aria-label="启用 Memory Control Plane V1"
            onClick={() => setControlPlaneEnabled((v) => !v)}
            className={`relative h-6 w-11 shrink-0 rounded-full transition-colors ${
              controlPlaneEnabled ? 'bg-jade' : 'bg-black/15'
            }`}
          >
            <span
              className={`absolute top-0.5 h-5 w-5 rounded-full bg-white shadow transition-transform ${
                controlPlaneEnabled ? 'translate-x-5' : 'translate-x-0.5'
              }`}
            />
          </button>
        </div>

        {/* Recall mode radio */}
        <div className="mb-4">
          <div className="mb-1.5 text-[12px] font-semibold text-foreground/85">检索模式 (recall_mode)</div>
          <p className="mb-2 text-[11px] text-muted-foreground">
            <span className="font-mono">lexical</span> 仅词法检索；<span className="font-mono">hybrid</span> 启用向量 + FTS + 情景融合
          </p>
          <div className="flex gap-2" role="radiogroup" aria-label="Recall mode">
            {(['lexical', 'hybrid'] as MemoryRecallMode[]).map((mode) => (
              <button
                key={mode}
                type="button"
                role="radio"
                aria-checked={recallMode === mode}
                onClick={() => setRecallMode(mode)}
                className={`flex-1 rounded-xl border px-3 py-2 text-[12px] font-medium transition-all ${
                  recallMode === mode
                    ? 'border-jade/40 bg-jade/10 text-jade'
                    : 'border-black/[0.09] bg-black/[0.02] text-foreground/65 hover:border-black/[0.18]'
                }`}
              >
                <div className="font-mono text-[11.5px] font-semibold">{mode}</div>
                <div className="mt-0.5 text-[10.5px] opacity-75">
                  {mode === 'lexical' ? '保守，无 embedding 依赖' : '推荐，多源融合排序'}
                </div>
              </button>
            ))}
          </div>
        </div>

        {/* Policy enforce mode radio */}
        <div>
          <div className="mb-1.5 text-[12px] font-semibold text-foreground/85">策略执行模式 (policy_enforce_mode)</div>
          <p className="mb-2 text-[11px] text-muted-foreground">
            <span className="font-mono">shadow</span> 仅审计不阻断；<span className="font-mono">enforce</span> 拒绝违规写入
          </p>
          <div className="flex gap-2" role="radiogroup" aria-label="Policy enforce mode">
            {(['shadow', 'enforce'] as MemoryPolicyEnforceMode[]).map((mode) => (
              <button
                key={mode}
                type="button"
                role="radio"
                aria-checked={policyEnforceMode === mode}
                onClick={() => setPolicyEnforceMode(mode)}
                className={`flex-1 rounded-xl border px-3 py-2 text-[12px] font-medium transition-all ${
                  policyEnforceMode === mode
                    ? 'border-jade/40 bg-jade/10 text-jade'
                    : 'border-black/[0.09] bg-black/[0.02] text-foreground/65 hover:border-black/[0.18]'
                }`}
              >
                <div className="font-mono text-[11.5px] font-semibold">{mode}</div>
                <div className="mt-0.5 text-[10.5px] opacity-75">
                  {mode === 'shadow' ? '默认，灰度阶段使用' : '生产强制，写入受限'}
                </div>
              </button>
            ))}
          </div>
        </div>
      </SettingsSurface>

      {/* ── Trajectory Export ── */}
      <SettingsSurface className="px-5 py-4">
        <div className="mb-1 flex items-center justify-between">
          <div>
            <div className="text-[10.5px] font-semibold uppercase tracking-widest text-black/30 mb-1">
              轨迹导出
            </div>
            <p className="text-[11.5px] text-muted-foreground">
              导出 Agent 运行轨迹记录，当前有{' '}
              <span className="font-semibold text-foreground/70">{trajectoryCount}</span> 条
            </p>
          </div>
          <button
            type="button"
            disabled={exporting}
            onClick={handleExport}
            className="flex h-8 items-center gap-1.5 rounded-xl border border-black/[0.09] bg-black/[0.025] px-3.5 text-[12px] font-medium transition-colors hover:bg-black/[0.05] disabled:cursor-not-allowed disabled:opacity-40"
          >
            <Download className={`h-3.5 w-3.5 ${exporting ? 'animate-spin' : ''}`} />
            {exporting ? '导出中…' : '导出轨迹'}
          </button>
        </div>
      </SettingsSurface>

      {/* ── Error ── */}
      {error && (
        <div className="flex items-start gap-2.5 rounded-xl border border-red-200/70 bg-red-50 px-4 py-3 text-[11.5px] text-red-700">
          <AlertTriangle className="mt-0.5 h-3.5 w-3.5 shrink-0" />
          {error}
        </div>
      )}
    </div>
  )
}
