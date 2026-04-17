import { useState, useEffect, useCallback } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { toast } from 'sonner'
import { SettingsSurface } from '../components/SettingsSurface'
import { CompactInput } from '../components/CompactInput'
import { Brain, Cpu, MessageSquare, Wrench, FileText, Zap, ChevronDown } from 'lucide-react'

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
  utility: '轻工具模型（摘要/翻译）',
  utility_large: '重工具模型（复杂推理）',
  summarizer: '摘要模型（记忆编译）',
  compiler: '编译模型（快速响应）',
}

/**
 * Dropdown for selecting a model from available options.
 * Shows "provider / model" format with grouping.
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

  const selectedLabel = value || '未设置'

  return (
    <div className="relative">
      <button
        type="button"
        onClick={() => setOpen(!open)}
        onBlur={() => setTimeout(() => setOpen(false), 150)}
        className="flex h-8 w-full items-center justify-between rounded-lg border border-border/60 bg-white/60 px-3 text-[12px] hover:bg-white/80 transition-colors"
      >
        <span className="truncate">{selectedLabel}</span>
        <ChevronDown className="h-3 w-3 text-muted-foreground shrink-0 ml-2" />
      </button>
      {open && (
        <div className="absolute z-50 mt-1 w-full rounded-lg border border-border/60 bg-white/95 shadow-lg backdrop-blur-sm max-h-60 overflow-y-auto">
          {/* Clear option */}
          <button
            type="button"
            onClick={() => { onChange(null); setOpen(false); }}
            className={`w-full px-3 py-2 text-left text-[12px] hover:bg-muted/50 transition-colors ${
              !value ? 'bg-muted/30 font-medium' : 'text-muted-foreground'
            }`}
          >
            未设置（使用默认）
          </button>
          {/* Model options grouped by provider */}
          {groups.map((group) => (
            <div key={group.provider_id}>
              <div className="px-3 py-1 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground bg-muted/20">
                {group.provider_name}
              </div>
              {group.models.map((model) => {
                const ref = `${group.provider_id}/${model.model_id}`
                return (
                  <button
                    key={ref}
                    type="button"
                    onClick={() => { onChange(ref); setOpen(false); }}
                    className={`w-full px-3 py-1.5 text-left text-[12px] hover:bg-muted/50 transition-colors flex items-center justify-between ${
                      value === ref ? 'bg-muted/30 font-medium' : ''
                    }`}
                  >
                    <span className="truncate">{model.model_id}</span>
                    {model.context_window && (
                      <span className="text-[10px] text-muted-foreground ml-2 shrink-0">
                        {(model.context_window / 1000).toFixed(0)}K ctx
                      </span>
                    )}
                  </button>
                )
              })}
            </div>
          ))}
          {groups.length === 0 || groups.every((g) => g.models.length === 0) && (
            <div className="px-3 py-3 text-[12px] text-muted-foreground text-center">
              暂无已配置的模型，请先在 Onboarding 中配置 Provider
            </div>
          )}
        </div>
      )}
    </div>
  )
}

export function ModelSettingsPage() {
  // Embedded model state
  const [modelName, setModelName] = useState(DEFAULT_MODEL_NAME)
  const [savingEmbedded, setSavingEmbedded] = useState(false)

  // Role model state
  const [roleConfigs, setRoleConfigs] = useState<ModelRoleConfig[]>([])
  const [modelGroups, setModelGroups] = useState<AvailableModelGroup[]>([])
  const [savingRole, setSavingRole] = useState<string | null>(null)

  // Load embedded model config
  const loadEmbeddedConfig = useCallback(async () => {
    try {
      const config = await invoke<ModelConfig>('get_model_config')
      setModelName(config.embedded_model_name)
    } catch {
      // Use default
    }
  }, [])

  // Load role model configs
  const loadRoleConfigs = useCallback(async () => {
    try {
      const roles = await invoke<ModelRoleConfig[]>('model_get_role_config')
      // Ensure all 5 roles exist
      const allRoles = ['chat', 'utility', 'utility_large', 'summarizer', 'compiler']
      const merged = allRoles.map((role) => {
        const existing = roles.find((r) => r.role === role)
        return existing || { role, model_ref: null }
      })
      setRoleConfigs(merged)
    } catch {
      // No config yet — use empty roles
      setRoleConfigs([
        { role: 'chat', model_ref: null },
        { role: 'utility', model_ref: null },
        { role: 'utility_large', model_ref: null },
        { role: 'summarizer', model_ref: null },
        { role: 'compiler', model_ref: null },
      ])
    }
  }, [])

  // Load available models
  const loadModels = useCallback(async () => {
    try {
      const groups = await invoke<AvailableModelGroup[]>('model_list_available')
      setModelGroups(groups)
    } catch {
      // No configured models yet
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
    // Update local state immediately for responsive UI
    setRoleConfigs((prev) =>
      prev.map((r) => (r.role === role ? { ...r, model_ref: modelRef } : r))
    )

    if (!modelRef) return // Clearing is instant, no API call needed

    setSavingRole(role)
    try {
      await invoke('model_set_role_config', { role, modelRef })
      toast.success(`已设置 ${ROLE_LABELS[role] || role}`)
    } catch (err) {
      toast.error('设置失败', { description: String(err) })
      // Rollback on error
      loadRoleConfigs()
    } finally {
      setSavingRole(null)
    }
  }

  return (
    <div className="flex flex-col gap-3">
      {/* ── LLM Model Roles ─────────────────────────────────────── */}
      <div className="flex items-center gap-3 px-1">
        <div className="flex size-8 items-center justify-center rounded-xl bg-primary/10">
          <Cpu className="size-4 text-primary" />
        </div>
        <div>
          <div className="text-[15px] font-semibold tracking-tight">LLM 模型角色</div>
          <div className="text-[12px] text-muted-foreground">
            为不同场景设置不同的模型，实现性能与成本的最佳平衡。
          </div>
        </div>
      </div>

      {/* Role assignments */}
      {roleConfigs.map(({ role, model_ref }) => {
        const Icon = ROLE_ICONS[role] || Cpu
        return (
          <SettingsSurface key={role} className="px-5 py-4">
            <div className="flex flex-col gap-3">
              <div className="flex items-center gap-3">
                <div className="flex size-7 items-center justify-center rounded-lg bg-muted/50">
                  <Icon className="size-3.5 text-muted-foreground" />
                </div>
                <div className="flex-1">
                  <div className="text-[13px] font-semibold tracking-tight">{ROLE_LABELS[role] || role}</div>
                  <div className="text-[11px] text-muted-foreground">
                    {role === 'chat' && '用于主对话和复杂交互'}
                    {role === 'utility' && '用于轻量工具调用、摘要和翻译'}
                    {role === 'utility_large' && '用于复杂推理和多步任务'}
                    {role === 'summarizer' && '用于记忆摘要和文本压缩'}
                    {role === 'compiler' && '用于记忆编译和快速响应'}
                  </div>
                </div>
                {savingRole === role && (
                  <div className="text-[11px] text-muted-foreground">保存中…</div>
                )}
              </div>
              <ModelDropdown
                value={model_ref}
                onChange={(ref) => void handleRoleChange(role, ref)}
                groups={modelGroups}
              />
            </div>
          </SettingsSurface>
        )
      })}

      {/* ── Embedded Vectorization Model ────────────────────────── */}
      <div className="border-t border-border/40 my-1" />
      <div className="flex items-center gap-3 px-1">
        <div className="flex size-8 items-center justify-center rounded-xl bg-primary/10">
          <Brain className="size-4 text-primary" />
        </div>
        <div>
          <div className="text-[15px] font-semibold tracking-tight">向量化模型</div>
          <div className="text-[12px] text-muted-foreground">
            配置本地 multilingual-e5-small 向量化模型名称。
          </div>
        </div>
      </div>

      <SettingsSurface className="px-5 py-4">
        <div className="flex flex-col gap-4">
          <div>
            <div className="text-[13px] font-semibold tracking-tight mb-1">模型名称</div>
            <div className="text-[12px] text-muted-foreground leading-5">
              当前使用的本地向量化模型。默认值为{' '}
              <code className="rounded bg-muted px-1 py-0.5 text-[11px] font-mono">{DEFAULT_MODEL_NAME}</code>
            </div>
          </div>

          <div className="flex items-end gap-3">
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
              className="h-8 shrink-0 rounded-xl border border-border/60 bg-white/60 px-4 text-[12px] font-medium shadow-none hover:bg-white/80 disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
            >
              {savingEmbedded ? '保存中…' : '保存'}
            </button>
          </div>
        </div>
      </SettingsSurface>

      {/* Info card */}
      <SettingsSurface className="px-5 py-4">
        <div className="text-[13px] font-semibold tracking-tight mb-3">关于向量化模型</div>
        <div className="flex flex-col gap-2 text-[12px] text-muted-foreground leading-5">
          <p>
            if2AI 使用{' '}
            <code className="rounded bg-muted px-1 text-[11px] font-mono">fastembed-rs</code>{' '}
            进行本地文本向量化，支持 100+ 语言（中/英/日/韩等）。
          </p>
          <p>
            模型在首次运行时自动下载并缓存到{' '}
            <code className="rounded bg-muted px-1 text-[11px] font-mono">~/.if2ai/models/fastembed/</code>
            ，之后离线可用。
          </p>
          <p>
            修改模型名称后需重启应用以生效。请确保新名称与{' '}
            <code className="rounded bg-muted px-1 text-[11px] font-mono">fastembed</code>{' '}
            支持的模型一致。
          </p>
        </div>
      </SettingsSurface>
    </div>
  )
}
