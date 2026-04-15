import React, { useCallback, useEffect, useRef, useState } from 'react'
import { Eye, EyeOff, GripVertical, Minus, Shield } from 'lucide-react'
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
    hint: 'Google 结果代理，质量高，每月 2500 次免费',
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
    hint: 'Google Gemini 搜索接地，需 Google AI Studio Key（即将支持）',
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

// ── ProviderRow (drag-to-reorder) ────────────────────────────────────────────

interface ProviderRowProps {
  entry: WebSearchProviderEntry
  index: number
  total: number
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
        'flex items-center gap-3 rounded-xl px-3 py-2.5 transition-all duration-150',
        isDragging ? 'opacity-40 scale-[0.98]' : 'opacity-100',
        isOver ? 'ring-1 ring-black/20 bg-black/[0.03]' : 'bg-white/60',
        'cursor-default',
      ]
        .filter(Boolean)
        .join(' ')}
    >
      {/* Drag handle */}
      <div className="cursor-grab text-black/20 hover:text-black/40 active:cursor-grabbing">
        <GripVertical className="h-4 w-4" />
      </div>

      {/* Priority badge */}
      <div className="flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-black/8 text-[11px] font-semibold text-black/40">
        {index + 1}
      </div>

      {/* Provider name */}
      <div className="min-w-[72px] text-[13px] font-medium text-black/80">{meta.name}</div>

      {/* Key / URL preview */}
      <div className="flex-1 text-[12px] font-mono text-black/40 truncate">{displayValue}</div>

      {/* Remove button */}
      <button
        type="button"
        onClick={() => onRemove(entry.id)}
        className="flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-rose-500 text-white transition-opacity hover:bg-rose-600"
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
  const [validateMsg, setValidateMsg] = useState<{ ok: boolean; text: string } | null>(null)
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

  // Clear feedback after 4 s
  useEffect(() => {
    if (!validateMsg) return
    const t = window.setTimeout(() => setValidateMsg(null), 4000)
    return () => clearTimeout(t)
  }, [validateMsg])

  const handleValidate = async () => {
    const value = inputValue.trim()
    if (!value) {
      inputRef.current?.focus()
      return
    }
    setValidating(true)
    setValidateMsg(null)
    try {
      const apiKey = selected.needsKey ? value : undefined
      const baseUrl = selected.needsUrl ? value : undefined
      const msg = await validateWebSearchKey(selectedId, apiKey, baseUrl)
      setValidateMsg({ ok: true, text: msg })

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
    } catch (err) {
      setValidateMsg({ ok: false, text: String(err) })
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
    try {
      setProviders(await removeWebSearchProvider(id))
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
      const updated = await reorderWebSearchProviders(reordered.map((p) => p.id))
      setProviders(updated)
    } catch {/* optimistic update stands */}
  }

  return (
    <div className="flex flex-col gap-3.5">
      {/* ── Add provider card ── */}
      <SettingsSurface className="p-5">
        <div className="space-y-4">
          <div className="space-y-1">
            <div className="text-[14px] font-semibold tracking-tight">添加搜索服务商</div>
            <p className="text-[12px] leading-5 text-muted-foreground">
              选择服务商后粘贴 API Key，点击盾牌图标验证后自动加入列表。
              未配置时将使用 DuckDuckGo 免费搜索（结果质量有限）。
            </p>
          </div>

          {/* Provider selector + input row */}
          <div className="flex items-center gap-2">
            {/* Provider dropdown */}
            <div className="relative">
              <select
                value={selectedId}
                onChange={(e) => {
                  setSelectedId(e.target.value)
                  setInputValue('')
                  setValidateMsg(null)
                }}
                className="h-9 appearance-none rounded-xl border border-black/10 bg-white/70 pl-3 pr-8 text-[13px] font-medium text-black/80 shadow-none outline-none focus:ring-1 focus:ring-black/20 cursor-pointer"
              >
                {PROVIDER_CATALOG.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.name}
                  </option>
                ))}
              </select>
              {/* custom chevron */}
              <div className="pointer-events-none absolute right-2.5 top-1/2 -translate-y-1/2 text-black/30">
                <svg width="10" height="6" viewBox="0 0 10 6" fill="none">
                  <path d="M1 1l4 4 4-4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"/>
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
                className="h-9 w-full rounded-xl border border-black/10 bg-white/70 py-0 pl-3 pr-[5.5rem] text-[13px] font-mono text-black/80 placeholder:text-black/28 outline-none focus:ring-1 focus:ring-black/20 disabled:opacity-40 disabled:cursor-not-allowed"
              />

              {/* Right icons: eye + shield */}
              <div className="absolute right-1 top-1/2 -translate-y-1/2 flex items-center gap-0.5">
                {selected.needsKey && (
                  <button
                    type="button"
                    onClick={() => setShowKey((v) => !v)}
                    className="flex h-7 w-7 items-center justify-center rounded-lg text-black/28 hover:bg-black/5 hover:text-black/50 transition-colors"
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
                  className="flex h-7 w-7 items-center justify-center rounded-lg border border-black/10 bg-white/80 text-black/40 shadow-sm transition-all hover:border-black/20 hover:text-black/70 disabled:opacity-30 disabled:cursor-not-allowed"
                  aria-label="验证 API Key"
                  style={{
                    outline: validateMsg?.ok
                      ? '2px solid rgb(34 197 94 / 0.6)'
                      : validateMsg && !validateMsg.ok
                      ? '2px solid rgb(239 68 68 / 0.6)'
                      : undefined,
                  }}
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
            <p className="text-[11px] leading-4 text-muted-foreground">{selected.hint}</p>
          )}

          {/* Validation feedback */}
          {validateMsg && (
            <div
              className={[
                'rounded-lg px-3 py-2 text-[12px] leading-5',
                validateMsg.ok
                  ? 'bg-emerald-50 text-emerald-700'
                  : 'bg-rose-50 text-rose-600',
              ].join(' ')}
            >
              {validateMsg.text}
            </div>
          )}

          {/* DDG notice when no providers configured */}
          {!loading && providers.length === 0 && (
            <div className="rounded-xl border border-amber-200 bg-amber-50 px-3.5 py-3 text-[12px] leading-5 text-amber-700">
              <span className="font-medium">当前使用 DuckDuckGo 免费搜索</span>，搜索质量有限，
              建议添加 Tavily 等服务商以获得更准确的结果。
            </div>
          )}
        </div>
      </SettingsSurface>

      {/* ── Configured providers list ── */}
      {providers.length > 0 && (
        <SettingsSurface className="p-4">
          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <div className="text-[13px] font-semibold tracking-tight text-black/70">
                已配置服务商
              </div>
              <div className="text-[11px] text-muted-foreground">拖拽调整优先级</div>
            </div>

            <div
              className="flex flex-col gap-1.5"
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
                  total={providers.length}
                  onRemove={(id) => void handleRemove(id)}
                  onDragStart={handleDragStart}
                  onDragOver={handleDragOver}
                  onDrop={() => void handleDrop()}
                  draggingIndex={draggingIndex}
                  overIndex={overIndex}
                />
              ))}
            </div>
          </div>
        </SettingsSurface>
      )}

      {/* ── Fallback info ── */}
      <SettingsSurface className="p-5">
        <div className="space-y-3">
          <div className="text-[13px] font-semibold tracking-tight text-black/70">
            搜索降级策略
          </div>
          <div className="space-y-1.5 text-[12px] leading-6 text-muted-foreground">
            <div className="flex items-start gap-2">
              <span className="mt-0.5 flex h-4 w-4 shrink-0 items-center justify-center rounded-full bg-black/8 text-[10px] font-semibold text-black/40">1</span>
              <span>优先使用列表中最高优先级（第一个）已启用的服务商</span>
            </div>
            <div className="flex items-start gap-2">
              <span className="mt-0.5 flex h-4 w-4 shrink-0 items-center justify-center rounded-full bg-black/8 text-[10px] font-semibold text-black/40">2</span>
              <span>服务商调用失败时，自动降级到 DuckDuckGo 即时问答 API（无需 Key）</span>
            </div>
            <div className="flex items-start gap-2">
              <span className="mt-0.5 flex h-4 w-4 shrink-0 items-center justify-center rounded-full bg-black/8 text-[10px] font-semibold text-black/40">3</span>
              <span>DDG 无结果时进一步降级到 DuckDuckGo Lite 页面抓取</span>
            </div>
          </div>
        </div>
      </SettingsSurface>
    </div>
  )
}
