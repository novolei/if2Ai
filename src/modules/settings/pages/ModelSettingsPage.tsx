import { useState, useEffect, useCallback, useRef } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { toast } from 'sonner'
import { SettingsSurface } from '../components/SettingsSurface'
import { CompactInput } from '../components/CompactInput'
import {
  Brain, Cpu, MessageSquare, Wrench, FileText, Zap, Globe,
  ChevronDown, Check, AlertCircle, RefreshCw,
} from 'lucide-react'
import { cn } from '@/lib/utils'
import { TtsModelSection } from './TtsModelSection'
import { ThinkingModeChip } from '@/components/chat/ThinkingModeChip'

const DEFAULT_MODEL_NAME = 'intfloat/multilingual-e5-small'

interface ModelConfig {
  embedded_model_name: string
  hf_mirror_url?: string | null
}

/** Known HuggingFace mirror presets */
const MIRROR_PRESETS = [
  { label: 'HuggingFace (官方)', value: '' },
  { label: 'hf-mirror.com (国内)', value: 'https://hf-mirror.com' },
]

interface ModelRoleConfig {
  role: string
  model_ref: string | null
}

interface AvailableModelGroup {
  provider_id: string
  provider_name: string
  models: AvailableModelEntry[]
}

interface AvailableModelEntry {
  model_id: string
  name: string
  context_window?: number
  /** P-MULTI-API — surfaced from the backend capability resolver. */
  reasoning?: boolean
  reasoning_required_in_tool_calls?: boolean
  supports_reasoning_effort?: boolean
}

const ROLE_META: Record<string, {
  label: string
  desc: string
  icon: typeof MessageSquare
  accentClass: string
}> = {
  chat: {
    label: '主对话模型',
    desc: '主对话 / 复杂交互',
    icon: MessageSquare,
    accentClass: 'bg-jade/[0.09] text-jade',
  },
  utility: {
    label: '轻工具模型',
    desc: '摘要 / 翻译 / 轻量调用',
    icon: Wrench,
    accentClass: 'bg-blue-500/[0.09] text-blue-600',
  },
  utility_large: {
    label: '重工具模型',
    desc: '复杂推理 / 多步任务',
    icon: Zap,
    accentClass: 'bg-violet-500/[0.09] text-violet-600',
  },
  summarizer: {
    label: '摘要模型',
    desc: '记忆摘要 / 文本压缩',
    icon: FileText,
    accentClass: 'bg-amber-500/[0.09] text-amber-600',
  },
  compiler: {
    label: '编译模型',
    desc: '记忆编译 / 快速响应',
    icon: Brain,
    accentClass: 'bg-rose-500/[0.09] text-rose-600',
  },
}

function SectionLabel({ children }: { children: React.ReactNode }) {
  return (
    <div className="mb-3 text-[10.5px] font-semibold uppercase tracking-widest text-black/30">
      {children}
    </div>
  )
}

// ── Model Dropdown ────────────────────────────────────────────────────────────

interface ModelDropdownProps {
  value: string | null
  onChange: (ref: string | null) => void
  groups: AvailableModelGroup[]
  isOpen: boolean
  onOpen: () => void
  onClose: () => void
  containerRef: (el: HTMLDivElement | null) => void
}

function ModelDropdown({
  value,
  onChange,
  groups,
  isOpen,
  onOpen,
  onClose,
  containerRef,
}: ModelDropdownProps) {
  const selectedLabel = value ? value.split('/').slice(1).join('/') : '未设置'
  const providerLabel = value ? value.split('/')[0] : null

  const hasModels = groups.some((g) => g.models.length > 0)

  return (
    <div ref={containerRef} className="relative">
      <button
        type="button"
        onClick={() => (isOpen ? onClose() : onOpen())}
        className={cn(
          'flex h-8 w-full items-center justify-between gap-2 rounded-xl border px-2.5 text-[12px] font-medium transition-all',
          isOpen
            ? 'border-jade/30 bg-jade/[0.04] ring-[2px] ring-jade/15'
            : 'border-black/[0.09] bg-black/[0.025] hover:bg-black/[0.04]',
        )}
      >
        <div className="flex min-w-0 items-center gap-1.5 truncate">
          {providerLabel && (
            <span className="shrink-0 rounded-md bg-black/[0.06] px-1.5 py-0.5 text-[9.5px] font-semibold text-black/40">
              {providerLabel}
            </span>
          )}
          <span
            className={cn(
              'truncate',
              value ? 'text-foreground/80' : 'text-muted-foreground/60',
            )}
          >
            {selectedLabel}
          </span>
        </div>
        <ChevronDown
          className={cn(
            'h-3 w-3 shrink-0 text-black/30 transition-transform duration-150',
            isOpen && 'rotate-180',
          )}
        />
      </button>

      {isOpen && (
        <div className="absolute left-0 z-[200] mt-1 max-h-60 w-full min-w-[200px] overflow-y-auto rounded-xl border border-black/[0.09] bg-white shadow-[0_8px_32px_rgba(0,0,0,0.14)]">
          {/* Clear option */}
          <button
            type="button"
            onClick={() => { onChange(null); onClose() }}
            className={cn(
              'flex w-full items-center gap-2 px-3 py-2 text-left text-[12px] transition-colors hover:bg-black/[0.03]',
              !value ? 'font-medium text-jade' : 'text-muted-foreground',
            )}
          >
            {!value ? (
              <Check className="h-3 w-3 shrink-0 text-jade" />
            ) : (
              <span className="h-3 w-3 shrink-0" />
            )}
            未设置（使用默认）
          </button>

          {/* Grouped models */}
          {groups.map((group) => (
            <div key={group.provider_id}>
              <div className="border-t border-black/[0.05] bg-black/[0.016] px-3 py-1 text-[9.5px] font-semibold uppercase tracking-widest text-black/30">
                {group.provider_name}
              </div>
              {group.models.map((model) => {
                const ref = `${group.provider_id}/${model.model_id}`
                const selected = value === ref
                return (
                  <button
                    key={ref}
                    type="button"
                    onClick={() => { onChange(ref); onClose() }}
                    className={cn(
                      'flex w-full items-center justify-between gap-2 px-3 py-1.5 text-left text-[12px] transition-colors hover:bg-black/[0.03]',
                      selected && 'text-jade',
                    )}
                  >
                    <div className="flex min-w-0 items-center gap-2">
                      {selected ? (
                        <Check className="h-3 w-3 shrink-0 text-jade" />
                      ) : (
                        <span className="h-3 w-3 shrink-0" />
                      )}
                      <span className={cn('truncate', selected && 'font-medium')}>
                        {model.model_id}
                      </span>
                    </div>
                    <div className="flex shrink-0 items-center gap-1.5">
                      <ThinkingModeChip model={model} compact />
                      {model.context_window && (
                        <span className="shrink-0 rounded-md bg-black/[0.05] px-1.5 text-[9.5px] text-black/35">
                          {(model.context_window / 1000).toFixed(0)}K
                        </span>
                      )}
                    </div>
                  </button>
                )
              })}
            </div>
          ))}

          {!hasModels && (
            <div className="flex flex-col items-center gap-1.5 px-3 py-4 text-center">
              <AlertCircle className="h-4 w-4 text-black/20" />
              <p className="text-[11.5px] text-muted-foreground">
                暂无已配置的模型
              </p>
              <p className="text-[10.5px] text-muted-foreground/60">
                请先在 Onboarding 中配置 Provider
              </p>
            </div>
          )}
        </div>
      )}
    </div>
  )
}

// ── Page ─────────────────────────────────────────────────────────────────────

export function ModelSettingsPage() {
  const [modelName, setModelName] = useState(DEFAULT_MODEL_NAME)
  const [savingEmbedded, setSavingEmbedded] = useState(false)
  const [roleConfigs, setRoleConfigs] = useState<ModelRoleConfig[]>([])
  const [modelGroups, setModelGroups] = useState<AvailableModelGroup[]>([])
  const [savingRole, setSavingRole] = useState<string | null>(null)
  const [loadingModels, setLoadingModels] = useState(true)

  /** HF mirror download source */
  const [mirrorUrl, setMirrorUrl] = useState('')
  const [savingMirror, setSavingMirror] = useState(false)

  /** Which role's dropdown is open — null means all closed */
  const [openRoleId, setOpenRoleId] = useState<string | null>(null)

  /** Refs to each dropdown container div, keyed by role */
  const dropdownRefs = useRef<Map<string, HTMLDivElement>>(new Map())

  // ── Close on outside click or ESC ────────────────────────────────────────
  useEffect(() => {
    if (!openRoleId) return

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setOpenRoleId(null)
    }
    const handleMouseDown = (e: MouseEvent) => {
      const ref = dropdownRefs.current.get(openRoleId)
      if (ref && !ref.contains(e.target as Node)) {
        setOpenRoleId(null)
      }
    }

    document.addEventListener('keydown', handleKeyDown)
    document.addEventListener('mousedown', handleMouseDown)
    return () => {
      document.removeEventListener('keydown', handleKeyDown)
      document.removeEventListener('mousedown', handleMouseDown)
    }
  }, [openRoleId])

  // ── Data loading ──────────────────────────────────────────────────────────
  const loadEmbeddedConfig = useCallback(async () => {
    try {
      const config = await invoke<ModelConfig>('get_model_config')
      setModelName(config.embedded_model_name)
      setMirrorUrl(config.hf_mirror_url ?? '')
    } catch {
      // Use default
    }
  }, [])

  const loadRoleConfigs = useCallback(async () => {
    try {
      const roles = await invoke<ModelRoleConfig[]>('model_get_role_config')
      const allRoles = ['chat', 'utility', 'utility_large', 'summarizer', 'compiler']
      const merged = allRoles.map((role) => {
        const existing = roles.find((r) => r.role === role)
        return existing || { role, model_ref: null }
      })
      setRoleConfigs(merged)
    } catch {
      setRoleConfigs([
        { role: 'chat', model_ref: null },
        { role: 'utility', model_ref: null },
        { role: 'utility_large', model_ref: null },
        { role: 'summarizer', model_ref: null },
        { role: 'compiler', model_ref: null },
      ])
    }
  }, [])

  const loadModels = useCallback(async () => {
    setLoadingModels(true)
    try {
      const groups = await invoke<AvailableModelGroup[]>('model_list_available')
      setModelGroups(groups)
    } catch {
      setModelGroups([])
    } finally {
      setLoadingModels(false)
    }
  }, [])

  useEffect(() => {
    void loadEmbeddedConfig()
    void loadRoleConfigs()
    void loadModels()
  }, [loadEmbeddedConfig, loadRoleConfigs, loadModels])

  const handleSaveEmbedded = async () => {
    if (!modelName.trim()) {
      toast.error('模型名称不能为空')
      return
    }
    setSavingEmbedded(true)
    try {
      await invoke<ModelConfig>('set_model_config', {
        config: {
          embedded_model_name: modelName.trim(),
          hf_mirror_url: mirrorUrl || null,
        },
      })
      toast.success('已保存向量化模型配置')
    } catch (err) {
      toast.error('保存失败', { description: String(err) })
    } finally {
      setSavingEmbedded(false)
    }
  }

  const handleRoleChange = async (role: string, modelRef: string | null) => {
    setRoleConfigs((prev) =>
      prev.map((r) => (r.role === role ? { ...r, model_ref: modelRef } : r)),
    )
    if (!modelRef) return
    setSavingRole(role)
    try {
      await invoke('model_set_role_config', { role, modelRef })
      toast.success(`已设置 ${ROLE_META[role]?.label ?? role}`)
      // Notify chat surfaces so the model dropdown updates immediately
      // when the user changes the active "chat" role here.
      window.dispatchEvent(new CustomEvent('if2ai:models-changed'))
    } catch (err) {
      toast.error('设置失败', { description: String(err) })
      void loadRoleConfigs()
    } finally {
      setSavingRole(null)
    }
  }

  const configuredCount = roleConfigs.filter((r) => r.model_ref !== null).length

  return (
    <div className="flex flex-col gap-3">
      {/* ── LLM Role Assignments ── */}
      <SettingsSurface className="overflow-visible px-5 py-4">
        <div className="mb-4 flex items-center justify-between">
          <div>
            <SectionLabel>LLM 模型角色</SectionLabel>
            <p className="text-[11.5px] text-muted-foreground">
              为不同场景分配最合适的模型，兼顾性能与成本
            </p>
          </div>
          <div className="flex items-center gap-2">
            {/* Configured count badge */}
            <span className={cn(
              'rounded-xl px-2.5 py-1 text-[11px] font-semibold',
              configuredCount === roleConfigs.length
                ? 'bg-jade/10 text-jade'
                : configuredCount > 0
                  ? 'bg-amber-50 text-amber-600'
                  : 'bg-black/[0.04] text-black/35',
            )}>
              {configuredCount}/{roleConfigs.length} 已配置
            </span>
            <button
              type="button"
              onClick={() => void loadModels()}
              className="flex h-7 w-7 items-center justify-center rounded-xl border border-black/[0.09] bg-black/[0.025] text-black/35 transition-colors hover:bg-black/[0.05] hover:text-black/60"
              aria-label="刷新模型列表"
            >
              <RefreshCw className={cn('h-3 w-3', loadingModels && 'animate-spin')} />
            </button>
          </div>
        </div>

        {/* Role rows */}
        <div className="flex flex-col divide-y divide-black/[0.05]">
          {roleConfigs.map(({ role, model_ref }) => {
            const meta = ROLE_META[role] ?? { label: role, desc: '', icon: Cpu, accentClass: 'bg-black/[0.06] text-black/40' }
            const Icon = meta.icon
            const isConfigured = model_ref !== null

            return (
              <div
                key={role}
                className={cn(
                  'flex items-center gap-3 py-3 transition-colors first:pt-0 last:pb-0',
                )}
              >
                {/* Status dot */}
                <div
                  className={cn(
                    'mt-0.5 h-1.5 w-1.5 shrink-0 self-start rounded-full',
                    isConfigured ? 'bg-jade' : 'bg-black/[0.15]',
                  )}
                />

                {/* Icon */}
                <div
                  className={cn(
                    'flex size-8 shrink-0 items-center justify-center rounded-xl',
                    meta.accentClass,
                  )}
                >
                  <Icon className="size-3.5" />
                </div>

                {/* Label + desc */}
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="text-[12.5px] font-semibold tracking-tight">
                      {meta.label}
                    </span>
                    {savingRole === role && (
                      <span className="flex items-center gap-1 text-[10.5px] text-muted-foreground">
                        <RefreshCw className="h-2.5 w-2.5 animate-spin" />
                        保存中
                      </span>
                    )}
                  </div>
                  <div className="text-[11px] text-muted-foreground">{meta.desc}</div>
                </div>

                {/* Dropdown */}
                <div className="w-[200px] shrink-0">
                  <ModelDropdown
                    value={model_ref}
                    onChange={(ref) => void handleRoleChange(role, ref)}
                    groups={modelGroups}
                    isOpen={openRoleId === role}
                    onOpen={() => setOpenRoleId(role)}
                    onClose={() => setOpenRoleId(null)}
                    containerRef={(el) => {
                      if (el) dropdownRefs.current.set(role, el)
                      else dropdownRefs.current.delete(role)
                    }}
                  />
                </div>
              </div>
            )
          })}
        </div>
      </SettingsSurface>

      {/* ── Embedded vectorization model ── */}
      <SettingsSurface className="px-5 py-4">
        <div className="mb-3 flex items-center gap-2.5">
          <div className="flex size-8 items-center justify-center rounded-xl bg-violet-500/[0.09]">
            <Brain className="size-4 text-violet-600" />
          </div>
          <div>
            <SectionLabel>向量化模型</SectionLabel>
          </div>
        </div>
        <p className="mb-4 text-[11.5px] leading-5 text-muted-foreground">
          配置本地{' '}
          <code className="rounded-md bg-black/[0.05] px-1.5 py-0.5 text-[11px] font-mono">
            multilingual-e5-small
          </code>{' '}
          向量化模型名称。修改后需重启应用生效。
        </p>
        <div className="flex items-end gap-2.5">
          <div className="flex-1">
            <CompactInput
              value={modelName}
              onChange={(e) => setModelName(e.target.value)}
              placeholder={DEFAULT_MODEL_NAME}
              label="模型名称"
            />
          </div>
          <button
            type="button"
            onClick={handleSaveEmbedded}
            disabled={savingEmbedded || modelName.trim() === DEFAULT_MODEL_NAME}
            className="h-8 shrink-0 rounded-xl bg-jade px-4 text-[12px] font-semibold text-white transition-colors hover:bg-jade/90 disabled:cursor-not-allowed disabled:opacity-40"
          >
            {savingEmbedded ? '保存中…' : '保存'}
          </button>
        </div>
      </SettingsSurface>

      {/* ── Download Source (HF Mirror) ── */}
      <SettingsSurface className="px-5 py-4">
        <div className="mb-3 flex items-center gap-2.5">
          <div className="flex size-8 items-center justify-center rounded-xl bg-blue-500/9">
            <Globe className="size-4 text-blue-600" />
          </div>
          <div>
            <SectionLabel>模型下载源</SectionLabel>
          </div>
        </div>
        <p className="mb-4 text-[11.5px] leading-5 text-muted-foreground">
          选择模型下载的服务器源。国内用户可选 hf-mirror.com 以获得更快的下载速度。
        </p>
        <div className="flex items-end gap-3">
          <div className="flex-1 flex flex-col gap-2">
            {MIRROR_PRESETS.map((preset) => (
              <button
                key={preset.value}
                type="button"
                onClick={() => setMirrorUrl(preset.value)}
                className={cn(
                  'flex items-center gap-3 rounded-xl border px-3.5 py-2.5 text-left transition-all',
                  (mirrorUrl === preset.value)
                    ? 'border-jade/30 bg-jade/4'
                    : 'border-black/6 bg-black/[0.016] hover:bg-black/3',
                )}
              >
                <div className={cn(
                  'flex size-4 shrink-0 items-center justify-center rounded-md border',
                  (mirrorUrl === preset.value)
                    ? 'border-jade bg-jade text-white'
                    : 'border-black/10',
                )}>
                  {mirrorUrl === preset.value && <Check className="h-2.5 w-2.5" />}
                </div>
                <span className="text-[12px] font-medium text-foreground/80">{preset.label}</span>
              </button>
            ))}
          </div>
          <button
            type="button"
            onClick={async () => {
              setSavingMirror(true)
              try {
                await invoke<ModelConfig>('set_model_config', {
                  config: {
                    embedded_model_name: modelName,
                    hf_mirror_url: mirrorUrl || null,
                  },
                })
                toast.success('已保存下载源配置')
              } catch (err) {
                toast.error('保存失败', { description: String(err) })
              } finally {
                setSavingMirror(false)
              }
            }}
            disabled={savingMirror}
            className="h-8 shrink-0 rounded-xl bg-jade px-4 text-[12px] font-semibold text-white transition-colors hover:bg-jade/90 disabled:cursor-not-allowed disabled:opacity-40"
          >
            {savingMirror ? '保存中…' : '保存'}
          </button>
        </div>
      </SettingsSurface>

      {/* ── Info card ── */}
      <SettingsSurface className="px-5 py-4">
        <SectionLabel>说明</SectionLabel>
        <div className="flex flex-col gap-2 text-[11.5px] leading-[1.6] text-muted-foreground">
          <p>
            if2AI 使用{' '}
            <code className="rounded bg-black/5 px-1 text-[11px] font-mono">fastembed-rs</code>{' '}
            进行本地文本向量化，支持 100+ 语言（中/英/日/韩等）。
          </p>
          <p>
            模型首次运行时自动下载并缓存至{' '}
            <code className="rounded bg-black/5 px-1 text-[11px] font-mono">
              ~/.if2ai/models/fastembed/
            </code>
            ，之后离线可用。
          </p>
        </div>
      </SettingsSurface>

      {/* ── TTS Model Section ── */}
      <TtsModelSection />
    </div>
  )
}
