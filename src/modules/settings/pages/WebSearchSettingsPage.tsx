import React, { useCallback, useEffect, useRef, useState } from 'react'
import { toast } from 'sonner'
import { Eye, EyeOff, GripVertical, Minus, Shield, AlertCircle } from 'lucide-react'
import { SettingsSurface } from '../components/SettingsSurface'
import {
  getWebSearchConfig,
  removeWebSearchProvider,
  reorderWebSearchProviders,
  upsertWebSearchProvider,
  validateWebSearchKey,
  type WebSearchProviderEntry,
} from '@/lib/tauri'

// ── Provider catalog ─────────────────────────────────────────────────────────

interface ProviderMeta {
  id: string
  name: string
  needsKey: boolean
  needsUrl: boolean
  keyPlaceholder: string
  urlPlaceholder?: string
  hint: string
}

const PROVIDER_CATALOG: ProviderMeta[] = [
  {
    id: 'tavily',
    name: 'Tavily',
    needsKey: true,
    needsUrl: false,
    keyPlaceholder: '粘贴 API Key…',
    hint: 'AI-native 搜索引擎，免费 1000 次/月，推荐首选',
  },
  {
    id: 'searxng',
    name: 'SearXNG',
    needsKey: false,
    needsUrl: true,
    keyPlaceholder: '（无需 API Key）',
    urlPlaceholder: '实例地址，如 https://searx.be',
    hint: '开源自托管聚合搜索，无需 API Key',
  },
  {
    id: 'serper',
    name: 'Serper',
    needsKey: true,
    needsUrl: false,
    keyPlaceholder: '粘贴 API Key…',
    hint: 'Google 结果代理，每月 2500 次免费',
  },
  {
    id: 'brave',
    name: 'Brave',
    needsKey: true,
    needsUrl: false,
    keyPlaceholder: '粘贴 API Key…',
    hint: 'Brave 独立索引，注重隐私，每月 2000 次免费',
  },
  {
    id: 'gemini',
    name: 'Gemini',
    needsKey: true,
    needsUrl: false,
    keyPlaceholder: '粘贴 Google AI Studio API Key…',
    hint: 'Google Gemini 搜索（即将支持）',
  },
]

function getCatalog(id: string): ProviderMeta {
  return (
    PROVIDER_CATALOG.find((p) => p.id === id) ?? {
      id,
      name: id,
      needsKey: true,
      needsUrl: false,
      keyPlaceholder: '粘贴 API Key…',
      hint: '',
    }
  )
}

// ── Compact Provider Row ─────────────────────────────────────────────────────

interface ProviderRowProps {
  entry: WebSearchProviderEntry
  index: number
  onRemove: (id: string) => void
  onDragStart: (index: number) => void
  onDragOver: (index: number) => void
  onDrop: () => void
  draggingIndex: number | null
  overIndex: number | null
}

function ProviderRow({
  entry,
  index,
  onRemove,
  onDragStart,
  onDragOver,
  onDrop,
  draggingIndex,
  overIndex,
}: ProviderRowProps) {
  const meta = getCatalog(entry.id)
  const displayValue = entry.base_url ?? entry.key_preview ?? '—'

  const isDragging = draggingIndex === index
  const isOver = overIndex === index && draggingIndex !== index

  return (
    <div
      draggable
      onDragStart={() => onDragStart(index)}
      onDragOver={(e) => {
        e.preventDefault()
        onDragOver(index)
      }}
      onDrop={onDrop}
      className={[
        'group flex items-center gap-2.5 rounded-2xl px-3 py-2 transition-all duration-150',
        isDragging ? 'opacity-40 scale-[0.98]' : 'opacity-100',
        isOver ? 'ring-1 ring-black/15 bg-black/[0.02]' : 'bg-white/60',
        'cursor-default',
      ]
        .filter(Boolean)
        .join(' ')}
    >
      <div className="cursor-grab text-black/15 group-hover:text-black/30 active:cursor-grabbing">
        <GripVertical className="h-3.5 w-3.5" />
      </div>
      <div className="flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-primary/10 text-[10px] font-semibold text-primary">
        {index + 1}
      </div>
      <div className="min-w-[60px] text-[12px] font-medium text-foreground/80">{meta.name}</div>
      <div className="flex-1 truncate text-[11px] font-mono text-muted-foreground">{displayValue}</div>
      <button
        type="button"
        onClick={() => onRemove(entry.id)}
        className="flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-rose-100 text-rose-500 opacity-0 transition-all hover:bg-rose-200 group-hover:opacity-100"
        aria-label={`移除 ${meta.name}`}
      >
        <Minus className="h-3 w-3" strokeWidth={3} />
      </button>
    </div>
  )
}

// ── Main page ────────────────────────────────────────────────────────────────

export function WebSearchSettingsPage() {
  const [providers, setProviders] = useState<WebSearchProviderEntry[]>([])
  const [loading, setLoading] = useState(true)
  const [selectedId, setSelectedId] = useState<string>('tavily')
  const [inputValue, setInputValue] = useState('')
  const [showKey, setShowKey] = useState(false)
  const [validating, setValidating] = useState(false)
  const inputRef = useRef<HTMLInputElement>(null)

  // DnD state
  const [draggingIndex, setDraggingIndex] = useState<number | null>(null)
  const [overIndex, setOverIndex] = useState<number | null>(null)

  const selected = getCatalog(selectedId)

  const refresh = useCallback(async () => {
    try {
      setProviders(await getWebSearchConfig())
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    void refresh()
  }, [refresh])

  const handleValidate = async () => {
    const value = inputValue.trim()
    if (!value) {
      inputRef.current?.focus()
      return
    }
    setValidating(true)
    try {
      const apiKey = selected.needsKey ? value : undefined
      const baseUrl = selected.needsUrl ? value : undefined
      const msg = await validateWebSearchKey(selectedId, apiKey, baseUrl)

      // Auto-add on success
      const updated = await upsertWebSearchProvider(
        selectedId,
        selected.name,
        apiKey,
        baseUrl,
        true
      )
      setProviders(updated)
      setInputValue('')
      toast.success(`${selected.name} 已添加`, { description: msg })
    } catch (err) {
      toast.error(`${selected.name} 验证失败`, { description: String(err) })
    } finally {
      setValidating(false)
    }
  }

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter') {
      void handleValidate()
    }
  }

  const handleRemove = async (id: string) => {
    const name = getCatalog(id).name
    try {
      setProviders(await removeWebSearchProvider(id))
      toast.info(`${name} 已移除`)
    } catch {/* ignore */}
  }

  // ── DnD handlers ────────────────────────────────────────────────────────

  const handleDragStart = (index: number) => setDraggingIndex(index)
  const handleDragOver = (index: number) => setOverIndex(index)

  const handleDrop = async () => {
    if (draggingIndex === null || overIndex === null || draggingIndex === overIndex) {
      setDraggingIndex(null)
      setOverIndex(null)
      return
    }
    const reordered = [...providers]
    const [moved] = reordered.splice(draggingIndex, 1)
    reordered.splice(overIndex, 0, moved)
    setProviders(reordered)
    setDraggingIndex(null)
    setOverIndex(null)
    try {
      await reorderWebSearchProviders(reordered.map((p) => p.id))
      toast.success('优先级已更新')
    } catch {/* optimistic update stands */}
  }

  return (
    <div className="flex flex-col gap-3">
      {/* Add provider */}
      <SettingsSurface className="px-5 py-4">
        <div className="text-[13px] font-semibold tracking-tight">添加搜索服务商</div>
        <p className="mt-1 text-[11px] leading-4 text-muted-foreground">
          选择服务商后粘贴 API Key，验证后自动加入列表
        </p>

        <div className="mt-3 flex items-center gap-2">
          {/* Provider selector */}
          <div className="relative">
            <select
              value={selectedId}
              onChange={(e) => {
                setSelectedId(e.target.value)
                setInputValue('')
              }}
              className="h-8 appearance-none rounded-xl border border-border/60 bg-white/60 pl-3 pr-7 text-[12px] font-medium text-foreground/80 outline-none focus:ring-1 focus:ring-primary/30 cursor-pointer"
            >
              {PROVIDER_CATALOG.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name}
                </option>
              ))}
            </select>
            <div className="pointer-events-none absolute right-2.5 top-1/2 -translate-y-1/2 text-black/30">
              <svg width="10" height="6" viewBox="0 0 10 6" fill="none">
                <path d="M1 1l4 4 4-4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
              </svg>
            </div>
          </div>

          {/* Key / URL input */}
          <div className="relative flex-1">
            <input
              ref={inputRef}
              type={showKey ? 'text' : 'password'}
              value={inputValue}
              onChange={(e) => setInputValue(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder={
                selected.needsUrl
                  ? (selected.urlPlaceholder ?? '实例 URL…')
                  : selected.keyPlaceholder
              }
              disabled={selectedId === 'gemini'}
              className="h-8 w-full rounded-xl border border-border/60 bg-white/60 py-0 pl-3 pr-[4.5rem] text-[12px] font-mono text-foreground/80 placeholder:text-muted-foreground/50 outline-none focus:ring-1 focus:ring-primary/30 disabled:opacity-40 disabled:cursor-not-allowed"
            />

            {/* Eye + validate */}
            <div className="absolute right-1 top-1/2 -translate-y-1/2 flex items-center gap-0.5">
              {selected.needsKey && (
                <button
                  type="button"
                  onClick={() => setShowKey((v) => !v)}
                  className="flex h-6 w-6 items-center justify-center rounded-lg text-muted-foreground/50 hover:bg-black/5 hover:text-foreground/70 transition-colors"
                  tabIndex={-1}
                  aria-label={showKey ? '隐藏 Key' : '显示 Key'}
                >
                  {showKey ? <EyeOff className="h-3.5 w-3.5" /> : <Eye className="h-3.5 w-3.5" />}
                </button>
              )}

              <button
                type="button"
                onClick={() => void handleValidate()}
                disabled={validating || !inputValue.trim() || selectedId === 'gemini'}
                className="flex h-6 w-6 items-center justify-center rounded-lg border border-border/50 bg-white/80 text-muted-foreground shadow-sm transition-all hover:text-foreground disabled:opacity-30 disabled:cursor-not-allowed"
                aria-label="验证 API Key"
              >
                {validating ? (
                  <div className="h-3 w-3 animate-spin rounded-full border border-black/30 border-t-transparent" />
                ) : (
                  <Shield className="h-3.5 w-3.5" />
                )}
              </button>
            </div>
          </div>
        </div>

        {/* Provider hint */}
        {selected.hint && (
          <p className="mt-2 text-[11px] leading-4 text-muted-foreground">{selected.hint}</p>
        )}

        {/* DDG notice when no providers configured */}
        {!loading && providers.length === 0 && (
          <div className="mt-3 flex items-start gap-2.5 rounded-2xl border border-amber-200/80 bg-amber-50 px-3.5 py-2.5 text-[11px] leading-4 text-amber-700">
            <AlertCircle className="mt-0.5 h-3.5 w-3.5 shrink-0" />
            <span>
              <span className="font-medium">当前使用 DuckDuckGo 免费搜索</span>，结果质量有限，
              建议添加 Tavily 等服务商
            </span>
          </div>
        )}
      </SettingsSurface>

      {/* Configured providers list */}
      {providers.length > 0 && (
        <SettingsSurface className="px-5 py-4">
          <div className="flex items-center justify-between">
            <div className="text-[13px] font-semibold tracking-tight">已配置服务商</div>
            <div className="text-[11px] text-muted-foreground">拖拽调整优先级</div>
          </div>
          <div
            className="mt-3 flex flex-col gap-1.5"
            onDragEnd={() => {
              setDraggingIndex(null)
              setOverIndex(null)
            }}
          >
            {providers.map((entry, index) => (
              <ProviderRow
                key={entry.id}
                entry={entry}
                index={index}
                onRemove={(id) => void handleRemove(id)}
                onDragStart={handleDragStart}
                onDragOver={handleDragOver}
                onDrop={() => void handleDrop()}
                draggingIndex={draggingIndex}
                overIndex={overIndex}
              />
            ))}
          </div>
        </SettingsSurface>
      )}

      {/* Fallback strategy */}
      <SettingsSurface className="px-5 py-4">
        <div className="text-[13px] font-semibold tracking-tight">搜索降级策略</div>
        <div className="mt-3 space-y-1.5 text-[11px] leading-4 text-muted-foreground">
          {[
            ['1', '优先使用列表中最高优先级的已启用服务商'],
            ['2', '服务商调用失败时，自动降级到 DuckDuckGo 即时问答'],
            ['3', 'DDG 无结果时进一步降级到 DuckDuckGo Lite 页面抓取'],
          ].map(([step, text]) => (
            <div key={step} className="flex items-start gap-2.5">
              <span className="mt-0.5 flex h-4 w-4 shrink-0 items-center justify-center rounded-full bg-primary/10 text-[9px] font-semibold text-primary">
                {step}
              </span>
              <span>{text}</span>
            </div>
          ))}
        </div>
      </SettingsSurface>
    </div>
  )
}
