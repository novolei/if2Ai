import { useCallback, useEffect, useState } from 'react'
import { toast } from 'sonner'
import {
  AlertTriangle,
  BookOpen,
  Brain,
  Check,
  Clock,
  ThumbsDown,
  Download,
  History,
  Sliders,
  Trash2,
  TrendingUp,
} from 'lucide-react'
import { SettingsSurface } from '../components/SettingsSurface'
import { MemoryAggregateOverview } from '@/components/memory/MemoryAggregateOverview'
import { LearnedTraitsPanel } from '@/components/memory/LearnedTraitsPanel'
import { PinnedMemoryEditor } from '@/components/memory/pinned/PinnedMemoryEditor'
import { CompiledMemoryViewer } from '@/components/memory/compiled/CompiledMemoryViewer'
import { MemoryNarrativeViewer } from '@/components/memory/narrative/MemoryNarrativeViewer'
import { MemoryDebugTab } from './MemoryDebugTab'
import { DayDreamPanel } from '@/components/memory/DayDreamPanel'
import { EvolutionTimeline } from '@/components/memory/EvolutionTimeline'
import { InsightPanel } from '@/components/memory/InsightPanel'
import { MemoryGraphPanel } from '@/components/memory/MemoryGraphPanel'
import {
  getMemoryConfig,
  setMemoryConfig,
  exportTrajectories,
  DEFAULT_PROMOTION_THRESHOLDS,
  type MemoryConfigInput,
  type MemoryRecallMode,
  type MemoryPolicyEnforceMode,
  type PromotionThresholds,
} from '@/lib/tauri'
import { memoryClearAll } from '@/api/memory'

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

  // Two-step confirm flow for the destructive "Clear all memories" action.
  // First click flips `clearArmed` true (button morphs to red "确认清空"),
  // second click within ~6s actually invokes the backend.  A timeout
  // disarms the button so a stray click later doesn't wipe the store.
  const [clearArmed, setClearArmed] = useState(false)
  const [clearing, setClearing] = useState(false)

  // Phase 8B Phase C verification — header tab switcher.
  // 'settings' renders the production memory configuration UI;
  // 'debug' renders MemoryDebugTab (一键 5 步编译测试).
  const [activeTab, setActiveTab] = useState<'settings' | 'daydream' | 'evolution' | 'graph' | 'debug'>('settings')

  // Memory Control Plane V1 feature flags (FE-B / FE-Settings).
  const [controlPlaneEnabled, setControlPlaneEnabled] = useState(true)
  const [recallMode, setRecallMode] = useState<MemoryRecallMode>('hybrid')
  const [policyEnforceMode, setPolicyEnforceMode] = useState<MemoryPolicyEnforceMode>('shadow')
  const [promotion, setPromotion] = useState<PromotionThresholds>(DEFAULT_PROMOTION_THRESHOLDS)

  // MEM-MOD-PD0 — Day Awareness controls.
  // Empty string means "no override" → backend falls back to OS local
  // (PD0 B-fix), then UTC. The detected OS zone is shown next to the
  // input so users see what an empty value will actually resolve to.
  const [timezone, setTimezone] = useState<string>('')
  const [cutoffHour, setCutoffHour] = useState<number>(4)
  const [detectedOsTimezone, setDetectedOsTimezone] = useState<string | null>(null)

  // Phase 8B.10 / T-UI-2 — modal state for the compiled memory viewer.
  const [compiledViewerOpen, setCompiledViewerOpen] = useState(false)
  // Phase 8B.11 / T-UI-3 — modal state for the session-summary timeline.
  const [narrativeViewerOpen, setNarrativeViewerOpen] = useState(false)

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
      setPromotion(cfg.promotion ?? DEFAULT_PROMOTION_THRESHOLDS)
      setTimezone(cfg.timezone ?? '')
      setCutoffHour(cfg.logical_day_cutoff_hour ?? 4)
      setDetectedOsTimezone(cfg.detected_os_timezone ?? null)
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
        promotion,
        timezone: timezone.trim() === '' ? null : timezone.trim(),
        logical_day_cutoff_hour: cutoffHour,
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

  /**
   * Two-step destructive flow.  First call arms; second call within the
   * 6-second auto-disarm window actually wipes the store.  We never
   * persist the confirmation across reloads.
   */
  const handleClearAll = useCallback(async () => {
    if (!clearArmed) {
      setClearArmed(true)
      window.setTimeout(() => setClearArmed(false), 6000)
      return
    }
    setClearing(true)
    setError(null)
    try {
      const removed = await memoryClearAll()
      toast.success('记忆已清空', {
        description:
          removed === 0 ? '没有可删除的条目' : `已删除 ${removed} 条记忆`,
      })
    } catch (e) {
      setError(String(e))
      toast.error('清空失败', { description: String(e) })
    } finally {
      setClearing(false)
      setClearArmed(false)
    }
  }, [clearArmed])

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
      {/* ── Tab switcher (Phase 8B verification UI) ── */}
      <div className="flex items-center gap-1 rounded-xl bg-black/[0.04] p-1 dark:bg-white/[0.04]">
        <button
          onClick={() => setActiveTab('settings')}
          className={`flex-1 rounded-lg px-3 py-1.5 text-[12px] font-medium transition-all ${
            activeTab === 'settings'
              ? 'bg-white text-foreground shadow-sm dark:bg-black/40'
              : 'text-muted-foreground hover:text-foreground'
          }`}
        >
          ⚙️ 设置
        </button>
        <button
          onClick={() => setActiveTab('daydream')}
          className={`flex-1 rounded-lg px-3 py-1.5 text-[12px] font-medium transition-all ${
            activeTab === 'daydream'
              ? 'bg-white text-foreground shadow-sm dark:bg-black/40'
              : 'text-muted-foreground hover:text-foreground'
          }`}
        >
          🌙 巩固
        </button>
        <button
          onClick={() => setActiveTab('evolution')}
          className={`flex-1 rounded-lg px-3 py-1.5 text-[12px] font-medium transition-all ${
            activeTab === 'evolution'
              ? 'bg-white text-foreground shadow-sm dark:bg-black/40'
              : 'text-muted-foreground hover:text-foreground'
          }`}
        >
          ✨ 进化
        </button>
        <button
          onClick={() => setActiveTab('graph')}
          className={`flex-1 rounded-lg px-3 py-1.5 text-[12px] font-medium transition-all ${
            activeTab === 'graph'
              ? 'bg-white text-foreground shadow-sm dark:bg-black/40'
              : 'text-muted-foreground hover:text-foreground'
          }`}
        >
          🕸️ 图谱
        </button>
        <button
          onClick={() => setActiveTab('debug')}
          className={`flex-1 rounded-lg px-3 py-1.5 text-[12px] font-medium transition-all ${
            activeTab === 'debug'
              ? 'bg-white text-foreground shadow-sm dark:bg-black/40'
              : 'text-muted-foreground hover:text-foreground'
          }`}
        >
          🧪 编译流水线测试 (Phase 8B)
        </button>
      </div>
      
      {activeTab === 'debug' ? (
        <MemoryDebugTab />
      ) : activeTab === 'daydream' ? (
        <DayDreamPanel />
      ) : activeTab === 'evolution' ? (
        <div className="flex flex-col gap-4">
          <EvolutionTimeline />
          <InsightPanel />
        </div>
      ) : activeTab === 'graph' ? (
        <div className="h-[600px]">
          <MemoryGraphPanel />
        </div>
      ) : (
        <>
          {/* ── Aggregate overview (Memory Audit P1 #8) ──
              Single hero panel answering "AI 记得我什么 — Pinned /
              Compiled / Facts / Summaries" so the four sources stop
              feeling like 4 disconnected sub-pages. */}
          <MemoryAggregateOverview
            onShowCompiled={() => setCompiledViewerOpen(true)}
            onShowSummaries={() => setNarrativeViewerOpen(true)}
          />

          {/* ── Pinned Memory (Phase 8A.12 / T-UI-1) ── */}
          <PinnedMemoryEditor />

      {/* ── Compiled memory.md viewer entry (Phase 8B.10 / T-UI-2) ── */}
      <SettingsSurface className="px-5 py-3">
        <button
          type="button"
          onClick={() => setCompiledViewerOpen(true)}
          className="flex w-full items-center justify-between rounded-lg border border-black/[0.06] bg-background/50 px-4 py-2.5 text-[12px] transition-colors hover:bg-black/[0.03] dark:hover:bg-white/[0.05]"
        >
          <span className="flex items-center gap-2">
            <BookOpen className="h-4 w-4 text-blue-600" />
            <span className="font-medium">查看 AI 长期记忆 (memory.md)</span>
          </span>
          <span className="text-[10.5px] text-muted-foreground">查看 →</span>
        </button>
      </SettingsSurface>

      <CompiledMemoryViewer
        open={compiledViewerOpen}
        onClose={() => setCompiledViewerOpen(false)}
        scope="global"
      />

      {/* ── Session summary timeline entry (Phase 8B.11 / T-UI-3) ── */}
      <SettingsSurface className="px-5 py-3">
        <button
          type="button"
          onClick={() => setNarrativeViewerOpen(true)}
          className="flex w-full items-center justify-between rounded-lg border border-black/[0.06] bg-background/50 px-4 py-2.5 text-[12px] transition-colors hover:bg-black/[0.03] dark:hover:bg-white/[0.05]"
        >
          <span className="flex items-center gap-2">
            <History className="h-4 w-4 text-purple-600" />
            <span className="font-medium">查看 Session 摘要时间线</span>
          </span>
          <span className="text-[10.5px] text-muted-foreground">查看 →</span>
        </button>
      </SettingsSurface>

      <MemoryNarrativeViewer
        open={narrativeViewerOpen}
        onClose={() => setNarrativeViewerOpen(false)}
        scope="global"
      />

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
        <p className="mb-3 text-[11.5px] text-muted-foreground">
          配置 Agent 上下文的 Token 分配比例
        </p>
        {/* Honest disclaimer (Memory Audit P0 #1): set_memory_config
            persists to memory_config.json but does NOT hot-reload the
            in-process ContextBudget — that's loaded once from
            ~/.if2ai/budget.yaml at startup. So changes here only take
            effect after the next App restart. */}
        <div className="mb-4 flex gap-2 rounded-lg border border-amber-300/40 bg-amber-50/50 px-3 py-2 text-[11px] leading-5 text-amber-900/85">
          <AlertTriangle className="mt-0.5 h-3.5 w-3.5 shrink-0 text-amber-600" />
          <div>
            <div className="font-semibold">改动需重启 App 后生效</div>
            <div className="mt-0.5 text-amber-900/65">
              此处的 Token 总数与槽位百分比会保存到
              <span className="mx-0.5 font-mono">~/.if2ai/memory_config.json</span>，
              但运行时的 ContextBudget 在 App 启动时从
              <span className="ml-0.5 font-mono">~/.if2ai/budget.yaml</span> 一次性加载，
              不支持热更新。当前会话仍按启动时的预算执行。
            </div>
          </div>
        </div>

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
        <p className="mb-3 text-[11.5px] text-muted-foreground">
          控制混合检索、策略执行模式与新一代记忆管线总开关
        </p>
        {/* Honest disclaimer (Memory Audit P0 #1): the V1 toggle and
            recall_mode radio are persisted but the runtime currently
            ignores them — VectorMemoryProvider is always preferred and
            ActiveRetrievalManager always runs the 3-fold RRF pipeline.
            This banner exists so the UI stops "lying"; we'll either
            wire them up in P1 or remove the fields. */}
        <div className="mb-4 flex gap-2 rounded-lg border border-amber-300/40 bg-amber-50/50 px-3 py-2 text-[11px] leading-5 text-amber-900/85">
          <AlertTriangle className="mt-0.5 h-3.5 w-3.5 shrink-0 text-amber-600" />
          <div>
            <div className="font-semibold">实验字段 · 当前未参与运行时分支</div>
            <div className="mt-0.5 text-amber-900/65">
              下方 <span className="font-mono">Control Plane V1</span> 开关与
              <span className="mx-0.5 font-mono">recall_mode</span> 选择会持久化到
              <span className="ml-0.5 font-mono">~/.if2ai/memory_config.json</span>，
              但当前运行时固定走「VectorMemoryProvider 优先 + 3-fold RRF 召回」路径，
              此设置仅作记录。计划在后续 slice 中真正接入或下架。
            </div>
          </div>
        </div>

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

      {/* ── Memory Promotion thresholds ── */}
      <SettingsSurface className="px-5 py-4">
        <div className="mb-3 flex items-center gap-2.5">
          <div className="flex size-7 items-center justify-center rounded-lg bg-amber-500/[0.1]">
            <TrendingUp className="h-3.5 w-3.5 text-amber-600" />
          </div>
          <div className="text-[10.5px] font-semibold uppercase tracking-widest text-black/30">
            记忆晋升阈值
          </div>
        </div>
        <p className="mb-4 text-[11.5px] text-muted-foreground">
          后台扫描器使用以下阈值推荐 <span className="font-mono">session→project</span> 与{' '}
          <span className="font-mono">project→global</span> 的记忆升级。仅生成候选；最终晋升仍需在
          Memory Browser 中手动确认。
        </p>

        <div className="grid grid-cols-2 gap-4">
          {/* session → project */}
          <div className="rounded-xl border border-violet-200/60 bg-violet-50/40 px-3 py-3">
            <div className="mb-2 text-[11px] font-semibold uppercase tracking-wider text-violet-700">
              session → project
            </div>
            <label className="mb-1 block text-[11px] text-foreground/70">
              最低访问次数 ({promotion.sessionToProjectAccess})
            </label>
            <input
              type="number"
              min={1}
              max={50}
              value={promotion.sessionToProjectAccess}
              onChange={(e) =>
                setPromotion((p) => ({
                  ...p,
                  sessionToProjectAccess: Math.max(1, Number(e.target.value) || 1),
                }))
              }
              className="mb-2 h-7 w-full rounded-lg border border-black/[0.09] bg-white/70 px-2 font-mono text-[12px] outline-none focus:border-violet-400/40 focus:ring-[2px] focus:ring-violet-400/15"
            />
            <label className="mb-1 block text-[11px] text-foreground/70">
              最低重要度 ({promotion.sessionToProjectImportance.toFixed(2)})
            </label>
            <input
              type="range"
              min={0}
              max={1}
              step={0.05}
              value={promotion.sessionToProjectImportance}
              onChange={(e) =>
                setPromotion((p) => ({
                  ...p,
                  sessionToProjectImportance: Number(e.target.value),
                }))
              }
              className="w-full accent-violet-500"
            />
          </div>

          {/* project → global */}
          <div className="rounded-xl border border-emerald-200/60 bg-emerald-50/40 px-3 py-3">
            <div className="mb-2 text-[11px] font-semibold uppercase tracking-wider text-emerald-700">
              project → global
            </div>
            <label className="mb-1 block text-[11px] text-foreground/70">
              最低访问次数 ({promotion.projectToGlobalAccess})
            </label>
            <input
              type="number"
              min={1}
              max={100}
              value={promotion.projectToGlobalAccess}
              onChange={(e) =>
                setPromotion((p) => ({
                  ...p,
                  projectToGlobalAccess: Math.max(1, Number(e.target.value) || 1),
                }))
              }
              className="mb-2 h-7 w-full rounded-lg border border-black/[0.09] bg-white/70 px-2 font-mono text-[12px] outline-none focus:border-emerald-400/40 focus:ring-[2px] focus:ring-emerald-400/15"
            />
            <label className="mb-1 block text-[11px] text-foreground/70">
              最低重要度 ({promotion.projectToGlobalImportance.toFixed(2)})
            </label>
            <input
              type="range"
              min={0}
              max={1}
              step={0.05}
              value={promotion.projectToGlobalImportance}
              onChange={(e) =>
                setPromotion((p) => ({
                  ...p,
                  projectToGlobalImportance: Number(e.target.value),
                }))
              }
              className="w-full accent-emerald-500"
            />
          </div>
        </div>

        <div className="mt-3 flex items-center justify-between">
          <button
            type="button"
            onClick={() => setPromotion(DEFAULT_PROMOTION_THRESHOLDS)}
            className="text-[11px] text-muted-foreground underline-offset-2 hover:underline"
          >
            重置为默认
          </button>
          <span className="font-mono text-[10.5px] text-muted-foreground">
            默认: 3/0.55 · 8/0.70
          </span>
        </div>
      </SettingsSurface>

      {/* ── Learned Traits (MEM-MOD-P7) ── */}
      <SettingsSurface className="px-5 py-4">
        <div className="mb-3 flex items-start justify-between gap-3">
          <div className="flex items-center gap-2.5">
            <div className="flex size-7 items-center justify-center rounded-lg bg-violet-500/[0.1]">
              <ThumbsDown className="h-3.5 w-3.5 text-violet-600" />
            </div>
            <div>
              <div className="text-[10.5px] font-semibold uppercase tracking-widest text-black/30">
                跨 session 学到的特征 · Learned Traits
              </div>
              <p className="mt-1 text-[11.5px] text-muted-foreground">
                Agent 在每个 session 结束时基于 reflection 提炼出 1–3 条「关于你」的累积观察。
                觉得不对的可以一键撤回，行（保留审计）但不再进入未来 prompt。
              </p>
            </div>
          </div>
        </div>
        <LearnedTraitsPanel />
      </SettingsSurface>

      {/* ── Day Awareness (MEM-MOD-PD0) ── */}
      <SettingsSurface className="px-5 py-4">
        <div className="mb-3 flex items-center gap-2.5">
          <div className="flex size-7 items-center justify-center rounded-lg bg-indigo-500/[0.1]">
            <Clock className="h-3.5 w-3.5 text-indigo-600" />
          </div>
          <div className="text-[10.5px] font-semibold uppercase tracking-widest text-black/30">
            时间感知 · Day Awareness
          </div>
        </div>
        <p className="mb-3 text-[11.5px] text-muted-foreground">
          决定 Agent 心中的「今天」从几点开始：默认 04:00 LOCAL 之前算前一天，避免「我熬到凌晨 3 点」的对话被误划进新一天。
          影响所有 daily-aggregation 记忆（compile_today / diary / 周报）。
        </p>

        {/*
          Banner is conditional now: only the *override* path needs a
          restart (`runtime::config` is `OnceLock`). When the user
          leaves both fields at their defaults (timezone = "", cutoff
          = 4) we silently follow the OS, no banner needed.
        */}
        {(timezone.trim() !== '' || cutoffHour !== 4) && (
          <div className="mb-3 flex gap-2 rounded-lg border border-amber-300/40 bg-amber-50/50 px-3 py-2 text-[11px] leading-5 text-amber-900/85">
            <AlertTriangle className="mt-0.5 h-3.5 w-3.5 shrink-0 text-amber-600" />
            <div>
              <div className="font-medium">非默认值需要重启生效</div>
              <div className="mt-0.5 text-amber-900/65">
                自定义的 <span className="font-mono">timezone</span> /{' '}
                <span className="font-mono">cutoff_hour</span> 由{' '}
                <span className="font-mono">runtime::config</span> 在启动时缓存为
                <span className="font-mono"> OnceLock</span>，下次启动时新值才会进入 prompt。
                留空 / 默认值则跟随系统、无需重启。
              </div>
            </div>
          </div>
        )}

        <div className="grid grid-cols-2 gap-3">
          <div>
            <label className="mb-1 block text-[11.5px] font-semibold text-foreground/85">
              时区 (IANA)
            </label>
            <input
              type="text"
              list="if2ai-tz-suggestions"
              value={timezone}
              placeholder={
                detectedOsTimezone
                  ? `留空 = 跟随系统 (${detectedOsTimezone})`
                  : '留空 = UTC（OS 探测失败）'
              }
              onChange={(e) => setTimezone(e.target.value)}
              className="h-8 w-full rounded-lg border border-black/[0.09] bg-black/[0.025] px-3 text-[12.5px] font-mono outline-none transition-all focus:border-indigo-400/40 focus:ring-[3px] focus:ring-indigo-400/15"
            />
            <datalist id="if2ai-tz-suggestions">
              <option value="Asia/Shanghai" />
              <option value="Asia/Tokyo" />
              <option value="Asia/Singapore" />
              <option value="Asia/Hong_Kong" />
              <option value="America/Los_Angeles" />
              <option value="America/New_York" />
              <option value="Europe/London" />
              <option value="Europe/Berlin" />
              <option value="UTC" />
            </datalist>
            <div className="mt-1 text-[10.5px] text-muted-foreground">
              {timezone.trim() === ''
                ? detectedOsTimezone
                  ? `当前生效：${detectedOsTimezone}（跟随系统，无需重启）`
                  : '当前生效：UTC（系统时区探测失败）'
                : '自定义 IANA 时区；解析失败回退到系统时区。'}
            </div>
          </div>
          <div>
            <label className="mb-1 block text-[11.5px] font-semibold text-foreground/85">
              一天起点 ({String(cutoffHour).padStart(2, '0')}:00)
            </label>
            <input
              type="range"
              min={0}
              max={23}
              step={1}
              value={cutoffHour}
              onChange={(e) => setCutoffHour(Number(e.target.value))}
              className="w-full accent-indigo-500"
            />
            <div className="mt-1 flex items-center justify-between text-[10.5px] text-muted-foreground">
              <span>00:00（自然日）</span>
              <span className="font-mono">默认 04:00</span>
              <span>23:00</span>
            </div>
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

      {/* ── Danger zone: wipe all memory ── */}
      <SettingsSurface className="border-red-200/70 bg-red-50/40 px-5 py-4">
        <div className="mb-1 flex items-center gap-2.5">
          <div className="flex size-7 items-center justify-center rounded-lg bg-red-500/[0.1]">
            <AlertTriangle className="h-3.5 w-3.5 text-red-600" />
          </div>
          <div className="text-[10.5px] font-semibold uppercase tracking-widest text-red-700/70">
            危险操作
          </div>
        </div>
        <p className="mb-3 text-[11.5px] text-red-700/80">
          一键清空所有记忆条目（包括 session / project / global 三层），
          此操作无法撤销。
        </p>
        <div className="flex items-center justify-between">
          <span className="font-mono text-[10.5px] text-red-700/60">
            {clearArmed ? '再次点击以确认 (6 秒内有效)' : ''}
          </span>
          <button
            type="button"
            disabled={clearing}
            onClick={handleClearAll}
            className={
              clearArmed
                ? 'flex h-8 items-center gap-1.5 rounded-xl bg-red-600 px-3.5 text-[12px] font-semibold text-white transition-colors hover:bg-red-700 disabled:cursor-not-allowed disabled:opacity-40'
                : 'flex h-8 items-center gap-1.5 rounded-xl border border-red-300/70 bg-white/70 px-3.5 text-[12px] font-medium text-red-700 transition-colors hover:bg-red-50 disabled:cursor-not-allowed disabled:opacity-40'
            }
          >
            <Trash2 className={`h-3.5 w-3.5 ${clearing ? 'animate-pulse' : ''}`} />
            {clearing ? '清空中…' : clearArmed ? '确认清空所有记忆' : '清空所有记忆'}
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
        </>
      )}
    </div>
  )
}
