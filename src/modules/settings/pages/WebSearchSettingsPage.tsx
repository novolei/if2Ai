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
import { cn } from '@/lib/utils'

// ── Provider catalog ─────────────────────────────────────────────────────────

interface ProviderMeta {
  id: string
  name: string
  needsKey: boolean
  needsUrl: boolean
  keyPlaceholder: string
  urlPlaceholder?: string
  hint: string
  color: string
}

const PROVIDER_CATALOG: ProviderMeta[] = [
  {
    id: 'tavily',
    name: 'Tavily',
    needsKey: true,
    needsUrl: false,
    keyPlaceholder: '粘贴 API Key…',
    hint: 'AI-native 搜索引擎，免费 1000 次/月，推荐首选',
    color: '#0ea5e9',
  },
  {
    id: 'searxng',
    name: 'SearXNG',
    needsKey: false,
    needsUrl: true,
    keyPlaceholder: '（无需 API Key）',
    urlPlaceholder: '实例地址，如 https://searx.be',
    hint: '开源自托管聚合搜索，无需 API Key',
    color: '#8b5cf6',
  },
  {
    id: 'serper',
    name: 'Serper',
    needsKey: true,
    needsUrl: false,
    keyPlaceholder: '粘贴 API Key…',
    hint: 'Google 结果代理，每月 2500 次免费',
    color: '#f97316',
  },
  {
    id: 'brave',
    name: 'Brave',
    needsKey: true,
    needsUrl: false,
    keyPlaceholder: '粘贴 API Key…',
    hint: 'Brave 独立索引，注重隐私，每月 2000 次免费',
    color: '#ef4444',
  },
  {
    id: 'gemini',
    name: 'Gemini',
    needsKey: true,
    needsUrl: false,
    keyPlaceholder: '粘贴 Google AI Studio API Key…',
    hint: 'Google Gemini 搜索（即将支持）',
    color: '#3b82f6',
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
      color: '#6b7280',
    }
  )
}

function SectionLabel({ children }: { children: React.ReactNode }) {
  return (
    <div className="mb-3 text-[10.5px] font-semibold uppercase tracking-widest text-black/30">
      {children}
    </div>
  )
}

// ── Draggable Provider Row ────────────────────────────────────────────────────

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
      onDragOver={(e) => { e.preventDefault(); onDragOver(index) }}
      onDrop={onDrop}
      className={cn(
        'group flex items-center gap-2.5 rounded-xl px-3 py-2 transition-all',
        isDragging ? 'opacity-40 scale-[0.98]' : 'opacity-100',
        isOver ? 'ring-1 ring-jade/20 bg-jade/[0.03]' : 'bg-black/[0.016]',
      )}
    >
      <div className="cursor-grab text-black/15 transition-colors group-hover:text-black/35 active:cursor-grabbing">
        <GripVertical className="h-3.5 w-3.5" />
      </div>
      <div
        className="flex h-5 w-5 shrink-0 items-center justify-center rounded-full text-[9.5px] font-bold text-white"
        style={{ background: meta.color }}
      >
        {index + 1}
      </div>
      <div className="min-w-[64px] text-[12px] font-semibold text-foreground/75">{meta.name}</div>
      <div className="flex-1 truncate font-mono text-[10.5px] text-muted-foreground">
        {displayValue}
      </div>
      <button
        type="button"
        onClick={() => onRemove(entry.id)}
        className="flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-rose-50 text-rose-400 opacity-0 transition-all hover:bg-rose-100 hover:text-rose-600 group-hover:opacity-100"
        aria-label={`移除 ${meta.name}`}
      >
        <Minus className="h-3 w-3" strokeWidth={3} />
      </button>
    </div>
  )
}

// ── Main Page ─────────────────────────────────────────────────────────────────

export function WebSearchSettingsPage() {
  const [providers, setProviders] = useState<WebSearchProviderEntry[]>([])
  const [loading, setLoading] = useState(true)
  const [selectedId, setSelectedId] = useState<string>('tavily')
  const [inputValue, setInputValue] = useState('')
  const [showKey, setShowKey] = useState(false)
  const [validating, setValidating] = useState(false)
  const inputRef = useRef<HTMLInputElement>(null)

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
      const updated = await upsertWebSearchProvider(selectedId, selected.name, apiKey, baseUrl, true)
      setProviders(updated)
      setInputValue('')
      toast.success(`${selected.name} 已添加`, { description: msg })
    } catch (err) {
      toast.error(`${selected.name} 验证失败`, { description: String(err) })
    } finally {
      setValidating(false)
    }
  }

  const handleRemove = async (id: string) => {
    const name = getCatalog(id).name
    try {
      setProviders(await removeWebSearchProvider(id))
      toast.info(`${name} 已移除`)
    } catch {
      // ignore
    }
  }

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
    } catch {
      // optimistic update stands
    }
  }

  return (
    <div className="flex flex-col gap-3">
      {/* ── Add provider ── */}
      <SettingsSurface className="px-5 py-4">
        <SectionLabel>添加搜索服务商</SectionLabel>
        <p className="mb-3 text-[11.5px] leading-4 text-muted-foreground">
          选择服务商后粘贴 API Key，回车或点击验证按钮自动加入列表
        </p>

        <div className="flex items-center gap-2">
          {/* Provider selector */}
          <div className="relative shrink-0">
            <select
              value={selectedId}
              onChange={(e) => {
                setSelectedId(e.target.value)
                setInputValue('')
              }}
              className="h-8 appearance-none rounded-xl border border-black/[0.09] bg-black/[0.025] pl-3 pr-7 text-[12px] font-semibold text-foreground/75 outline-none transition-colors focus:border-jade/40 focus:ring-[3px] focus:ring-jade/15"
              style={{ cursor: 'pointer' }}
            >
              {PROVIDER_CATALOG.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name}
                </option>
              ))}
            </select>
            <div className="pointer-events-none absolute right-2 top-1/2 -translate-y-1/2 text-black/30">
              <svg width="9" height="5" viewBox="0 0 10 6" fill="none">
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
              onKeyDown={(e) => { if (e.key === 'Enter') void handleValidate() }}
              placeholder={selected.needsUrl ? (selected.urlPlaceholder ?? '实例 URL…') : selected.keyPlaceholder}
              disabled={selectedId === 'gemini'}
              className="h-8 w-full rounded-xl border border-black/[0.09] bg-black/[0.025] pl-3 pr-[4rem] font-mono text-[12px] text-foreground/80 outline-none transition-all placeholder:font-sans placeholder:text-muted-foreground/40 focus:border-jade/40 focus:ring-[3px] focus:ring-jade/15 disabled:cursor-not-allowed disabled:opacity-40"
            />
            <div className="absolute right-1 top-1/2 flex -translate-y-1/2 items-center gap-0.5">
              {selected.needsKey && (
                <button
                  type="button"
                  onClick={() => setShowKey((v) => !v)}
                  tabIndex={-1}
                  aria-label={showKey ? '隐藏 Key' : '显示 Key'}
                  className="flex h-6 w-6 items-center justify-center rounded-lg text-black/25 transition-colors hover:bg-black/[0.05] hover:text-black/50"
                >
                  {showKey ? <EyeOff className="h-3.5 w-3.5" /> : <Eye className="h-3.5 w-3.5" />}
                </button>
              )}
              <button
                type="button"
                onClick={() => void handleValidate()}
                disabled={validating || !inputValue.trim() || selectedId === 'gemini'}
                aria-label="验证 API Key"
                className="flex h-6 w-6 items-center justify-center rounded-lg border border-black/[0.09] bg-white text-black/40 shadow-sm transition-all hover:text-black/70 disabled:cursor-not-allowed disabled:opacity-30"
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

        {/* Hint */}
        {selected.hint && (
          <p className="mt-2 text-[11px] text-muted-foreground">{selected.hint}</p>
        )}

        {/* No providers notice */}
        {!loading && providers.length === 0 && (
          <div className="mt-3 flex items-start gap-2.5 rounded-xl border border-amber-200/70 bg-amber-50 px-3.5 py-2.5 text-[11px] leading-4 text-amber-700">
            <AlertCircle className="mt-0.5 h-3.5 w-3.5 shrink-0" />
            <span>
              <span className="font-semibold">当前使用 DuckDuckGo 免费搜索</span>，结果质量有限，
              建议添加 Tavily 等服务商
            </span>
          </div>
        )}
      </SettingsSurface>

      {/* ── Configured providers ── */}
      {providers.length > 0 && (
        <SettingsSurface className="px-5 py-4">
          <div className="mb-3 flex items-center justify-between">
            <SectionLabel>已配置服务商</SectionLabel>
            <div className="mb-3 text-[10.5px] text-muted-foreground">拖拽调整优先级</div>
          </div>
          <div
            className="flex flex-col gap-1.5"
            onDragEnd={() => { setDraggingIndex(null); setOverIndex(null) }}
          >
            {providers.map((entry, index) => (
              <ProviderRow
                key={entry.id}
                entry={entry}
                index={index}
                onRemove={(id) => void handleRemove(id)}
                onDragStart={(i) => setDraggingIndex(i)}
                onDragOver={(i) => setOverIndex(i)}
                onDrop={() => void handleDrop()}
                draggingIndex={draggingIndex}
                overIndex={overIndex}
              />
            ))}
          </div>
        </SettingsSurface>
      )}

      {/* ── Fallback strategy ── */}
      <SettingsSurface className="px-5 py-4">
        <SectionLabel>搜索降级策略</SectionLabel>
        <div className="flex flex-col gap-2">
          {[
            ['1', '优先使用列表中最高优先级的已启用服务商'],
            ['2', '服务商调用失败时，自动降级到 DuckDuckGo 即时问答'],
            ['3', 'DDG 无结果时进一步降级到 DuckDuckGo Lite 页面抓取'],
          ].map(([step, text]) => (
            <div key={step} className="flex items-start gap-2.5">
              <span className="mt-0.5 flex h-4 w-4 shrink-0 items-center justify-center rounded-full bg-jade/10 text-[9px] font-bold text-jade">
                {step}
              </span>
              <span className="text-[11.5px] leading-5 text-muted-foreground">{text}</span>
            </div>
          ))}
        </div>
      </SettingsSurface>
    </div>
  )
}
