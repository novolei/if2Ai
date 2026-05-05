/**
 * MemoryGraphPanel — 记忆知识图谱力导向图可视化组件
 *
 * Interactive force-directed graph visualization of the memory knowledge graph.
 * Uses react-force-graph-2d for rendering with:
 * - Default full-graph loading (all nodes and links)
 * - Fuzzy search with highlight (matched nodes glow gold)
 * - Node size scaled by importance
 * - Node color by cognitive_layer
 * - Link color/style by link_type
 * - Directional arrows on links
 * - Node drag, zoom, pan interactions
 * - Click-to-select node detail panel
 * - Link type filter checkboxes
 */

import { useCallback, useEffect, useMemo, useRef, useState } from 'react'

import { toast } from 'sonner'
import { Loader2, Network, Search, Sparkles } from 'lucide-react'
import ForceGraph2D from 'react-force-graph-2d'
import {
  useFullMemoryGraph,
  memoryGraphDiscover,
  type FullGraphDto,
} from '@/api/memory'

// ── Types ────────────────────────────────────────────────────────────

interface ForceNode {
  id: string
  content: string
  category: string
  importance: number
  trustScore: number
  cognitiveLayer: number
  isHighlighted: boolean
  x?: number
  y?: number
}

interface ForceLink {
  source: string | ForceNode
  target: string | ForceNode
  linkType: string
}

interface ForceGraphData {
  nodes: ForceNode[]
  links: ForceLink[]
}

// ── Theme helpers ────────────────────────────────────────────────────

function useThemeKey() {
  const [themeKey, setThemeKey] = useState(0)
  useEffect(() => {
    const observer = new MutationObserver(() => {
      setThemeKey(k => k + 1)
    })
    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ['class'],
    })
    return () => observer.disconnect()
  }, [])
  return themeKey
}

// ── Color mappings ───────────────────────────────────────────────────

function cognitiveLayerColor(layer: number, isDark: boolean): string {
  switch (layer) {
    case 1: return isDark ? '#f87171' : '#dc2626'
    case 2: return isDark ? '#fbbf24' : '#d97706'
    case 3: return isDark ? '#34d399' : '#059669'
    case 4: return isDark ? '#a78bfa' : '#7c3aed'
    default: return isDark ? '#9ca3af' : '#4b5563'
  }
}

function linkTypeColor(type: string): string {
  switch (type) {
    case 'related_to': return '#94a3b8'
    case 'supersedes': return '#f59e0b'
    case 'contradicts': return '#ef4444'
    case 'evidence_for': return '#10b981'
    case 'consolidated_into': return '#3b82f6'
    case 'promoted_to': return '#8b5cf6'
    default: return '#d1d5db'
  }
}

// ── Data transform ───────────────────────────────────────────────────

function transformFullGraph(graph: FullGraphDto): ForceGraphData {
  const nodes: ForceNode[] = graph.nodes.map(n => ({
    id: n.key,
    content: n.content,
    category: n.category,
    importance: n.importance,
    trustScore: n.trust_score,
    cognitiveLayer: n.cognitive_layer,
    isHighlighted: false,
  }))

  const nodeSet = new Set(nodes.map(n => n.id))
  const linkSet = new Set<string>()
  const links: ForceLink[] = []

  for (const link of graph.links) {
    if (nodeSet.has(link.source_key) && nodeSet.has(link.target_key)) {
      const id = `${link.source_key}-${link.target_key}-${link.link_type}`
      if (!linkSet.has(id)) {
        linkSet.add(id)
        links.push({
          source: link.source_key,
          target: link.target_key,
          linkType: link.link_type,
        })
      }
    }
  }

  return { nodes, links }
}

// ── Sub-components ───────────────────────────────────────────────────

function LinkTypeFilter({
  allTypes,
  hidden,
  onChange,
}: {
  allTypes: string[]
  hidden: Set<string>
  onChange: (s: Set<string>) => void
}) {
  if (allTypes.length === 0) return null

  const toggle = (type: string) => {
    const next = new Set(hidden)
    if (next.has(type)) next.delete(type)
    else next.add(type)
    onChange(next)
  }

  return (
    <div className="flex items-center gap-2 flex-wrap text-xs">
      <span className="text-muted-foreground">过滤:</span>
      {allTypes.map(type => (
        <label key={type} className="flex items-center gap-1 cursor-pointer">
          <input
            type="checkbox"
            checked={!hidden.has(type)}
            onChange={() => toggle(type)}
          />
          <span style={{ color: linkTypeColor(type) }}>{type}</span>
        </label>
      ))}
    </div>
  )
}

function Legend({ isDark }: { isDark: boolean }) {
  const layers = [
    { label: 'L1 Reactive', color: cognitiveLayerColor(1, isDark) },
    { label: 'L2 Deliberative', color: cognitiveLayerColor(2, isDark) },
    { label: 'L3 Reflective', color: cognitiveLayerColor(3, isDark) },
    { label: 'L4 Meta', color: cognitiveLayerColor(4, isDark) },
    { label: '匹配高亮', color: '#fbbf24' },
  ]

  return (
    <div className="flex items-center gap-4 text-xs text-muted-foreground flex-wrap">
      {layers.map(l => (
        <div key={l.label} className="flex items-center gap-1">
          <div
            className="w-3 h-3 rounded-full"
            style={{ backgroundColor: l.color }}
          />
          <span>{l.label}</span>
        </div>
      ))}
    </div>
  )
}

// ── Main component ───────────────────────────────────────────────────

export function MemoryGraphPanel() {
  const [searchQuery, setSearchQuery] = useState('')
  const [hiddenLinkTypes, setHiddenLinkTypes] = useState<Set<string>>(new Set())
  const [selectedNode, setSelectedNode] = useState<ForceNode | null>(null)
  const [discovering, setDiscovering] = useState(false)

  const themeKey = useThemeKey()

  const themeColors = useMemo(() => {
    const isDark = document.documentElement.classList.contains('dark')
    return {
      canvasBg: isDark ? 'rgb(2, 6, 23)' : 'rgb(255, 255, 255)',
      textPrimary: isDark ? '#f1f5f9' : '#0f172a',
      textSecondary: isDark ? '#94a3b8' : '#64748b',
      labelBg: isDark ? 'rgba(15, 23, 42, 0.85)' : 'rgba(248, 250, 252, 0.9)',
      nodeBorder: isDark ? 'rgba(255, 255, 255, 0.3)' : 'rgba(0, 0, 0, 0.2)',
      matchText: isDark ? '#fde68a' : '#92400e',
      isDark,
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [themeKey])

  const { graph: fullGraph, loading, error, refetch } = useFullMemoryGraph()

  // Transform full graph data
  const rawGraphData = useMemo<ForceGraphData>(() => {
    if (!fullGraph) return { nodes: [], links: [] }
    return transformFullGraph(fullGraph)
  }, [fullGraph])

  // Apply link type filter
  const graphData = useMemo<ForceGraphData>(() => {
    if (hiddenLinkTypes.size === 0) return rawGraphData
    return {
      nodes: rawGraphData.nodes,
      links: rawGraphData.links.filter(l => !hiddenLinkTypes.has(l.linkType)),
    }
  }, [rawGraphData, hiddenLinkTypes])

  // Collect all link types for filter
  const allLinkTypes = useMemo(() => {
    const types = new Set<string>()
    for (const link of rawGraphData.links) {
      types.add(link.linkType)
    }
    return Array.from(types)
  }, [rawGraphData.links])

  // Fuzzy search matching
  const matchedNodeIds = useMemo(() => {
    if (!searchQuery.trim() || !graphData.nodes.length) return new Set<string>()
    const q = searchQuery.toLowerCase()
    return new Set(
      graphData.nodes
        .filter(n =>
          n.id.toLowerCase().includes(q) ||
          n.content.toLowerCase().includes(q) ||
          n.category.toLowerCase().includes(q)
        )
        .map(n => n.id)
    )
  }, [searchQuery, graphData.nodes])

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const fgRef = useRef<any>(null)
  const containerRef = useRef<HTMLDivElement>(null)
  const [dimensions, setDimensions] = useState({ width: 600, height: 400 })

  useEffect(() => {
    if (!containerRef.current) return
    const observer = new ResizeObserver(entries => {
      const { width, height } = entries[0].contentRect
      setDimensions({ width: Math.max(width, 300), height: Math.max(height, 300) })
    })
    observer.observe(containerRef.current)
    return () => observer.disconnect()
  }, [])

  // Auto-center to first matched node on search
  useEffect(() => {
    if (matchedNodeIds.size > 0 && fgRef.current) {
      const firstMatchId = matchedNodeIds.values().next().value
      const node = graphData.nodes.find(n => n.id === firstMatchId)
      if (node && node.x != null && node.y != null) {
        fgRef.current.centerAt(node.x, node.y, 500)
        fgRef.current.zoom(2, 500)
      }
    }
  }, [matchedNodeIds, graphData.nodes])

  const handleNodeClick = useCallback(
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (node: any) => {
      const n = node as ForceNode
      setSelectedNode(n)
      if (fgRef.current && node.x != null && node.y != null) {
        fgRef.current.centerAt(node.x, node.y, 500)
        fgRef.current.zoom(3, 500)
      }
    },
    [],
  )

  const handleDiscover = useCallback(async () => {
    setDiscovering(true)
    try {
      const newLinks = await memoryGraphDiscover()
      if (newLinks.length > 0) {
        toast.success(`发现 ${newLinks.length} 个新关系`)
        void refetch()
      } else {
        toast.info('未发现新关系')
      }
    } catch (e) {
      toast.error(`发现关系失败: ${String(e)}`)
    } finally {
      setDiscovering(false)
    }
  }, [refetch])

  const nodeCanvasObject = useCallback(
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (node: any, ctx: CanvasRenderingContext2D, globalScale: number) => {
      const n = node as ForceNode
      const isMatch = matchedNodeIds.has(n.id)
      const hasSearch = searchQuery.trim().length > 0
      const size = (2 + (n.importance ?? 0.5) * 8) * (isMatch ? 1.5 : 1)
      const alpha = hasSearch && !isMatch ? 0.3 : 1.0

      ctx.globalAlpha = alpha

      // Draw circle
      ctx.beginPath()
      ctx.arc(node.x!, node.y!, size, 0, 2 * Math.PI)
      ctx.fillStyle = cognitiveLayerColor(n.cognitiveLayer, themeColors.isDark)
      ctx.fill()

      // Node border for better contrast
      ctx.strokeStyle = themeColors.nodeBorder
      ctx.lineWidth = 1 / globalScale
      ctx.stroke()

      // Gold ring for matched nodes
      if (isMatch) {
        ctx.beginPath()
        ctx.arc(node.x!, node.y!, size + 1 / globalScale, 0, 2 * Math.PI)
        ctx.strokeStyle = '#fbbf24'
        ctx.lineWidth = 3 / globalScale
        ctx.stroke()
      }

      // Label with background (only show non-match labels when zoomed in enough)
      if (globalScale > 0.8 || isMatch) {
        const label = n.id.length > 12 ? n.id.slice(0, 12) + '...' : n.id
        const fontSize = Math.max(isMatch ? 9 : 8, (isMatch ? 10 : 9) / globalScale)
        ctx.font = `${fontSize}px -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif`

        const textWidth = ctx.measureText(label).width
        const padding = 2
        const labelY = node.y! + size + 2

        // Semi-transparent background for label
        ctx.fillStyle = themeColors.labelBg
        ctx.beginPath()
        const rx = 2 // border radius
        const bx = node.x! - textWidth / 2 - padding
        const by = labelY
        const bw = textWidth + padding * 2
        const bh = fontSize + padding * 2
        ctx.roundRect(bx, by, bw, bh, rx)
        ctx.fill()

        // Text color
        ctx.fillStyle = isMatch ? themeColors.matchText : themeColors.textPrimary
        ctx.textAlign = 'center'
        ctx.textBaseline = 'top'
        ctx.fillText(label, node.x!, labelY + padding)
      }

      ctx.globalAlpha = 1.0
    },
    [matchedNodeIds, searchQuery, themeColors],
  )

  const getLinkColor = useCallback(
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (link: any) => {
      const l = link as ForceLink
      const hasSearch = searchQuery.trim().length > 0
      const color = linkTypeColor(l.linkType)
      if (hasSearch) {
        const srcId = typeof l.source === 'string' ? l.source : l.source.id
        const tgtId = typeof l.target === 'string' ? l.target : l.target.id
        const srcMatch = matchedNodeIds.has(srcId)
        const tgtMatch = matchedNodeIds.has(tgtId)
        if (!srcMatch && !tgtMatch) return color + '4D' // 30% alpha
      }
      return color
    },
    [matchedNodeIds, searchQuery],
  )

  return (
    <div className="flex flex-col h-full gap-3">
      {/* Toolbar */}
      <div className="flex items-center gap-2 flex-wrap">
        <div className="relative flex-1 min-w-[200px]">
          <input
            value={searchQuery}
            onChange={e => setSearchQuery(e.target.value)}
            placeholder="搜索记忆内容、Key 或分类..."
            className="w-full px-3 py-1.5 pl-8 rounded-md border bg-background text-sm"
          />
          <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-muted-foreground" />
        </div>

        <button
          onClick={handleDiscover}
          disabled={discovering}
          className="flex items-center gap-1.5 px-3 py-1.5 rounded-md border text-sm hover:bg-accent disabled:opacity-50"
        >
          {discovering ? (
            <Loader2 className="h-4 w-4 animate-spin" />
          ) : (
            <Sparkles className="h-4 w-4" />
          )}
          {discovering ? '发现中...' : '发现关系'}
        </button>

        <span className="text-xs text-muted-foreground">
          {graphData.nodes.length} 个节点 · {graphData.links.length} 条链接
        </span>
      </div>

      {/* Search result stats */}
      {searchQuery.trim() && (
        <div className="text-xs text-muted-foreground">
          找到 <span className="text-amber-500 dark:text-amber-400 font-medium">{matchedNodeIds.size}</span> 个匹配节点（共 {graphData.nodes.length} 个）
        </div>
      )}

      <LinkTypeFilter
        allTypes={allLinkTypes}
        hidden={hiddenLinkTypes}
        onChange={setHiddenLinkTypes}
      />

      {/* Force graph container */}
      <div
        ref={containerRef}
        className="flex-1 relative rounded-lg border bg-background overflow-hidden min-h-[300px]"
      >
        {/* Loading overlay */}
        {loading && (
          <div className="absolute inset-0 flex items-center justify-center bg-background/60 z-40">
            <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />
            <span className="ml-2 text-sm text-muted-foreground">加载中...</span>
          </div>
        )}

        {/* Error overlay */}
        {error && (
          <div className="absolute inset-0 flex items-center justify-center z-40">
            <div className="rounded-lg border border-red-200 dark:border-red-800 bg-red-50 dark:bg-red-950/20 p-4 text-sm text-red-700 dark:text-red-300 max-w-[80%]">
              {error}
            </div>
          </div>
        )}

        {/* Empty state */}
        {!loading && !error && graphData.nodes.length === 0 && (
          <div className="absolute inset-0 flex flex-col items-center justify-center text-muted-foreground">
            <Network className="h-12 w-12 mb-3 opacity-40" />
            <p className="text-sm">暂无记忆数据</p>
            <p className="text-xs mt-1 opacity-70">创建记忆后，图谱将自动展示所有关联关系</p>
          </div>
        )}

        {/* Force graph */}
        {graphData.nodes.length > 0 && (
          <ForceGraph2D
            ref={fgRef}
            width={dimensions.width}
            height={dimensions.height}
            graphData={graphData}
            nodeLabel=""
            nodeVal={(node: ForceNode) => 2 + node.importance * 8}
            nodeCanvasObject={nodeCanvasObject}
            nodePointerAreaPaint={(node: ForceNode, color: string, ctx: CanvasRenderingContext2D) => {
              const size = 2 + (node.importance ?? 0.5) * 8
              ctx.beginPath()
              ctx.arc(node.x!, node.y!, size + 2, 0, 2 * Math.PI)
              ctx.fillStyle = color
              ctx.fill()
            }}
            linkColor={getLinkColor}
            linkDirectionalArrowLength={4}
            linkDirectionalArrowRelPos={1}
            linkWidth={() => 1.5}
            linkLineDash={(link: ForceLink) =>
              link.linkType === 'contradicts' ? [4, 2] : null
            }
            onNodeClick={handleNodeClick}
            cooldownTicks={100}
            warmupTicks={50}
            d3AlphaDecay={0.05}
            enableZoomInteraction={true}
            enablePanInteraction={true}
            enableNodeDrag={true}
            backgroundColor={themeColors.canvasBg}
            linkLabel={(link: ForceLink) => link.linkType}
          />
        )}

        {/* Selected node detail panel */}
        {selectedNode && (
          <div className="absolute bottom-2 left-2 right-2 p-3 rounded-lg border border-border bg-popover shadow-lg text-sm z-50">
            <div className="flex justify-between items-start">
              <div className="font-mono text-xs text-muted-foreground">{selectedNode.id}</div>
              <button onClick={() => setSelectedNode(null)} className="text-muted-foreground hover:text-foreground">✕</button>
            </div>
            <div className="text-foreground mt-1 line-clamp-3">{selectedNode.content}</div>
            <div className="mt-2 flex gap-3 text-xs text-muted-foreground">
              <span>分类: {selectedNode.category}</span>
              <span>重要度: {(selectedNode.importance * 100).toFixed(0)}%</span>
              <span>信任: {selectedNode.trustScore.toFixed(2)}</span>
              <span>层级: L{selectedNode.cognitiveLayer}</span>
            </div>
          </div>
        )}
      </div>

      <Legend isDark={themeColors.isDark} />
    </div>
  )
}
