import { useState, useEffect, useCallback } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { toast } from 'sonner'
import { SettingsSurface } from '../components/SettingsSurface'
import { CompactInput } from '../components/CompactInput'
import { Brain, Cpu, MessageSquare, Wrench, FileText, Zap, ChevronDown, Check } from 'lucide-react'
import { cn } from '@/lib/utils'

const DEFAULT_MODEL_NAME = 'intfloat/multilingual-e5-small'

interface ModelConfig {
  embedded_model_name: string
}

interface ModelRoleConfig {
  role: string
  model_ref: string | null
}

interface AvailableModelGroup {
  provider_id: string
  provider_name: string
  models: { model_id: string; name: string; context_window?: number }[]
}

const ROLE_ICONS: Record<string, typeof MessageSquare> = {
  chat: MessageSquare,
  utility: Wrench,
  utility_large: Zap,
  summarizer: FileText,
  compiler: Brain,
}

const ROLE_LABELS: Record<string, string> = {
  chat: '主对话模型',
  utility: '轻工具模型',
  utility_large: '重工具模型',
  summarizer: '摘要模型',
  compiler: '编译模型',
}

const ROLE_DESCS: Record<string, string> = {
  chat: '用于主对话和复杂交互',
  utility: '用于轻量工具调用、摘要和翻译',
  utility_large: '用于复杂推理和多步任务',
  summarizer: '用于记忆摘要和文本压缩',
  compiler: '用于记忆编译和快速响应',
}

function SectionLabel({ children }: { children: React.ReactNode }) {
  return (
    <div className="mb-3 text-[10.5px] font-semibold uppercase tracking-widest text-black/30">
      {children}
    </div>
  )
}

/**
 * Model dropdown with grouping. Styled consistently with the settings control system.
 */
function ModelDropdown({
  value,
  onChange,
  groups,
}: {
  value: string | null
  onChange: (ref: string | null) => void
  groups: AvailableModelGroup[]
}) {
  const [open, setOpen] = useState(false)
  const selectedLabel = value ?? '未设置'

  return (
    <div className="relative">
      <button
        type="button"
        onClick={() => setOpen(!open)}
        onBlur={() => setTimeout(() => setOpen(false), 150)}
        className="flex h-7 w-full items-center justify-between rounded-xl border border-black/[0.09] bg-black/[0.025] px-2.5 text-[12px] font-medium transition-colors hover:bg-black/[0.04]"
      >
        <span className="truncate text-foreground/80">{selectedLabel}</span>
        <ChevronDown
          className={cn('h-3 w-3 shrink-0 text-black/30 transition-transform', open && 'rotate-180')}
        />
      </button>
      {open && (
        <div className="absolute z-50 mt-1 max-h-56 w-full overflow-y-auto rounded-xl border border-black/[0.09] bg-white shadow-[0_8px_24px_rgba(0,0,0,0.12)]">
          {/* Clear option */}
          <button
            type="button"
            onClick={() => { onChange(null); setOpen(false) }}
            className={cn(
              'flex w-full items-center gap-2 px-3 py-2 text-left text-[12px] transition-colors hover:bg-black/[0.03]',
              !value ? 'text-foreground font-medium' : 'text-muted-foreground',
            )}
          >
            {!value && <Check className="h-3 w-3 text-jade" />}
            <span className={!value ? '' : 'ml-5'}>未设置（使用默认）</span>
          </button>
          {/* Grouped models */}
          {groups.map((group) => (
            <div key={group.provider_id}>
              <div className="border-t border-black/[0.05] px-3 py-1 text-[9.5px] font-semibold uppercase tracking-widest text-black/25">
                {group.provider_name}
              </div>
              {group.models.map((model) => {
                const ref = `${group.provider_id}/${model.model_id}`
                const selected = value === ref
                return (
                  <button
                    key={ref}
                    type="button"
                    onClick={() => { onChange(ref); setOpen(false) }}
                    className={cn(
                      'flex w-full items-center justify-between gap-2 px-3 py-1.5 text-left text-[12px] transition-colors hover:bg-black/[0.03]',
                      selected ? 'text-jade font-medium' : '',
                    )}
                  >
                    <div className="flex items-center gap-2 truncate">
                      {selected ? (
                        <Check className="h-3 w-3 shrink-0 text-jade" />
                      ) : (
                        <span className="w-3 shrink-0" />
                      )}
                      <span className="truncate">{model.model_id}</span>
                    </div>
                    {model.context_window && (
                      <span className="shrink-0 text-[10px] text-muted-foreground">
                        {(model.context_window / 1000).toFixed(0)}K
                      </span>
                    )}
                  </button>
                )
              })}
            </div>
          ))}
          {(groups.length === 0 || groups.every((g) => g.models.length === 0)) && (
            <div className="px-3 py-4 text-center text-[12px] text-muted-foreground">
              暂无已配置的模型，请先在 Onboarding 中配置 Provider
            </div>
          )}
        </div>
      )}
    </div>
  )
}

export function ModelSettingsPage() {
  const [modelName, setModelName] = useState(DEFAULT_MODEL_NAME)
  const [savingEmbedded, setSavingEmbedded] = useState(false)
  const [roleConfigs, setRoleConfigs] = useState<ModelRoleConfig[]>([])
  const [modelGroups, setModelGroups] = useState<AvailableModelGroup[]>([])
  const [savingRole, setSavingRole] = useState<string | null>(null)

  const loadEmbeddedConfig = useCallback(async () => {
    try {
      const config = await invoke<ModelConfig>('get_model_config')
      setModelName(config.embedded_model_name)
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
    try {
      const groups = await invoke<AvailableModelGroup[]>('model_list_available')
      setModelGroups(groups)
    } catch {
      setModelGroups([])
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
        config: { embedded_model_name: modelName.trim() },
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
      prev.map((r) => (r.role === role ? { ...r, model_ref: modelRef } : r))
    )
    if (!modelRef) return
    setSavingRole(role)
    try {
      await invoke('model_set_role_config', { role, modelRef })
      toast.success(`已设置 ${ROLE_LABELS[role] || role}`)
    } catch (err) {
      toast.error('设置失败', { description: String(err) })
      void loadRoleConfigs()
    } finally {
      setSavingRole(null)
    }
  }

  return (
    <div className="flex flex-col gap-3">
      {/* ── LLM roles ── */}
      <SettingsSurface className="px-5 py-4">
        <SectionLabel>LLM 模型角色</SectionLabel>
        <p className="mb-3 text-[11.5px] text-muted-foreground">
          为不同场景设置不同的模型，实现性能与成本的最佳平衡。
        </p>
        <div className="flex flex-col gap-2">
          {roleConfigs.map(({ role, model_ref }) => {
            const Icon = ROLE_ICONS[role] || Cpu
            return (
              <div
                key={role}
                className="flex items-center gap-3 rounded-xl border border-black/[0.06] bg-black/[0.016] px-4 py-2.5"
              >
                <div className="flex size-7 shrink-0 items-center justify-center rounded-lg bg-jade/[0.08]">
                  <Icon className="size-3.5 text-jade" />
                </div>
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="text-[12.5px] font-semibold tracking-tight">
                      {ROLE_LABELS[role] || role}
                    </span>
                    {savingRole === role && (
                      <span className="text-[10.5px] text-muted-foreground">保存中…</span>
                    )}
                  </div>
                  <div className="text-[11px] text-muted-foreground">{ROLE_DESCS[role]}</div>
                </div>
                <div className="w-[180px] shrink-0">
                  <ModelDropdown
                    value={model_ref}
                    onChange={(ref) => void handleRoleChange(role, ref)}
                    groups={modelGroups}
                  />
                </div>
              </div>
            )
          })}
        </div>
      </SettingsSurface>

      {/* ── Embedded model ── */}
      <SettingsSurface className="px-5 py-4">
        <div className="mb-3 flex items-center gap-2.5">
          <div className="flex size-7 items-center justify-center rounded-lg bg-jade/[0.08]">
            <Brain className="size-3.5 text-jade" />
          </div>
          <div>
            <SectionLabel>向量化模型</SectionLabel>
          </div>
        </div>
        <p className="mb-3 text-[11.5px] leading-5 text-muted-foreground">
          配置本地{' '}
          <code className="rounded-md bg-black/[0.05] px-1 text-[11px] font-mono">
            multilingual-e5-small
          </code>{' '}
          向量化模型名称。
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
            disabled={savingEmbedded || modelName === DEFAULT_MODEL_NAME}
            className="h-8 shrink-0 rounded-xl border border-black/[0.09] bg-jade px-4 text-[12px] font-medium text-white shadow-none transition-colors hover:bg-jade/90 disabled:cursor-not-allowed disabled:opacity-40"
          >
            {savingEmbedded ? '保存中…' : '保存'}
          </button>
        </div>
      </SettingsSurface>

      {/* ── Info ── */}
      <SettingsSurface className="px-5 py-4">
        <SectionLabel>关于向量化模型</SectionLabel>
        <div className="flex flex-col gap-2 text-[11.5px] leading-5 text-muted-foreground">
          <p>
            if2AI 使用{' '}
            <code className="rounded-md bg-black/[0.05] px-1 text-[11px] font-mono">fastembed-rs</code>{' '}
            进行本地文本向量化，支持 100+ 语言（中/英/日/韩等）。
          </p>
          <p>
            模型在首次运行时自动下载并缓存到{' '}
            <code className="rounded-md bg-black/[0.05] px-1 text-[11px] font-mono">
              ~/.if2ai/models/fastembed/
            </code>
            ，之后离线可用。
          </p>
          <p>
            修改模型名称后需重启应用以生效。请确保新名称与{' '}
            <code className="rounded-md bg-black/[0.05] px-1 text-[11px] font-mono">fastembed</code>{' '}
            支持的模型一致。
          </p>
        </div>
      </SettingsSurface>
    </div>
  )
}
