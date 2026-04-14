//! SkillsHubView — hub search interface with source filter tabs.
//!
//! Provides a unified interface for browsing skills across multiple hub sources
//! (GitHub, skills.sh, ClawHub, etc.) with search and source filtering.

import { useState, useMemo } from 'react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Badge } from '@/components/ui/badge'
import { cn } from '@/lib/utils'
import {
  Search,
  Globe,
  Filter,
  Star,
  Tag,
  CheckCircle,
  XCircle,
  Loader2,
} from 'lucide-react'
import type { HubSearchResult, TrustLevel } from './types'
import { TRUST_LABELS } from './types'

interface SkillsHubViewProps {
  /** Search results from hub sources */
  results: HubSearchResult[]
  /** Loading state */
  loading: boolean
  /** Error message */
  error: string | null
  /** Called when user initiates a search */
  onSearch: (query: string, sourceFilter: string | null) => void
  /** Called when user clicks install on a skill */
  onInstall: (skill: HubSearchResult) => void
  /** Currently installed skill names */
  installedSkillNames: Set<string>
  /** Current source filter */
  sourceFilter?: string | null
  /** Available source filters */
  availableSources?: string[]
  className?: string
}

const SOURCE_CONFIG: Record<
  string,
  { label: string; icon: typeof Globe; color: string }
> = {
  github: { label: 'GitHub', icon: Globe, color: 'text-gray-700' },
  'skills.sh': { label: 'skills.sh', icon: Globe, color: 'text-sky-600' },
  clawhub: { label: 'ClawHub', icon: Tag, color: 'text-purple-600' },
  marketplace: { label: 'Marketplace', icon: Star, color: 'text-amber-600' },
  builtin: { label: '内置', icon: CheckCircle, color: 'text-emerald-600' },
}

const TRUST_COLORS: Record<TrustLevel, { bg: string; text: string }> = {
  builtin: { bg: 'bg-emerald-100', text: 'text-emerald-700' },
  trusted: { bg: 'bg-blue-100', text: 'text-blue-700' },
  community: { bg: 'bg-gray-100', text: 'text-gray-700' },
  'agent-created': { bg: 'bg-purple-100', text: 'text-purple-700' },
}

export function SkillsHubView({
  results,
  loading,
  error,
  onSearch,
  onInstall,
  installedSkillNames,
  sourceFilter = null,
  availableSources = ['github', 'skills.sh', 'clawhub', 'marketplace'],
  className,
}: SkillsHubViewProps) {
  const [query, setQuery] = useState('')
  const [localSourceFilter, setLocalSourceFilter] = useState<string | null>(sourceFilter)
  const [selectedSkill, setSelectedSkill] = useState<HubSearchResult | null>(null)

  const handleSearch = () => {
    onSearch(query, localSourceFilter)
  }

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') {
      handleSearch()
    }
  }

  const filteredResults = useMemo(() => {
    return results.filter((result) => {
      if (localSourceFilter && result.source !== localSourceFilter) {
        return false
      }
      if (!query.trim()) return true
      const q = query.toLowerCase()
      return (
        result.name.toLowerCase().includes(q) ||
        result.description.toLowerCase().includes(q) ||
        result.tags.some((tag) => tag.toLowerCase().includes(q))
      )
    })
  }, [results, localSourceFilter, query])

  const groupedBySource = useMemo(() => {
    const groups: Record<string, HubSearchResult[]> = {}
    for (const result of filteredResults) {
      if (!groups[result.source]) {
        groups[result.source] = []
      }
      groups[result.source].push(result)
    }
    return groups
  }, [filteredResults])

  return (
    <div className={cn('space-y-4', className)}>
      {/* Search Bar */}
      <div className="flex gap-3">
        <div className="relative flex-1">
          <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
          <Input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="搜索技能名称、描述或标签..."
            className="pl-10"
          />
        </div>
        <Button onClick={handleSearch} disabled={loading}>
          {loading ? <Loader2 className="h-4 w-4 animate-spin" /> : '搜索'}
        </Button>
      </div>

      {/* Source Filters */}
      <div className="flex flex-wrap items-center gap-2">
        <Filter className="h-4 w-4 text-muted-foreground" />
        <span className="text-sm text-muted-foreground">来源:</span>
        <button
          type="button"
          onClick={() => setLocalSourceFilter(null)}
          className={cn(
            'rounded-full px-3 py-1 text-sm transition-colors',
            localSourceFilter === null
              ? 'bg-black/90 text-white'
              : 'bg-black/[0.04] hover:bg-black/[0.08]'
          )}
        >
          全部
        </button>
        {availableSources.map((source) => {
          const config = SOURCE_CONFIG[source] || {
            label: source,
            icon: Globe,
            color: 'text-gray-700',
          }
          const Icon = config.icon
          return (
            <button
              key={source}
              type="button"
              onClick={() => setLocalSourceFilter(source)}
              className={cn(
                'inline-flex items-center gap-1.5 rounded-full px-3 py-1 text-sm transition-colors',
                localSourceFilter === source
                  ? 'bg-black/90 text-white'
                  : 'bg-black/[0.04] hover:bg-black/[0.08]'
              )}
            >
              <Icon className={cn('h-3.5 w-3.5', localSourceFilter !== source && config.color)} />
              {config.label}
            </button>
          )
        })}
      </div>

      {/* Error State */}
      {error && (
        <div className="rounded-lg border border-red-200 bg-red-50 p-4">
          <p className="text-sm text-red-600">{error}</p>
        </div>
      )}

      {/* Loading State */}
      {loading && (
        <div className="flex items-center justify-center py-8">
          <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
          <span className="ml-2 text-sm text-muted-foreground">搜索中...</span>
        </div>
      )}

      {/* Results */}
      {!loading && !error && (
        <>
          <div className="text-sm text-muted-foreground">
            找到 {filteredResults.length} 个技能
            {localSourceFilter && ` (来源: ${SOURCE_CONFIG[localSourceFilter]?.label || localSourceFilter})`}
          </div>

          {filteredResults.length === 0 && query && (
            <div className="rounded-lg border border-black/8 bg-black/[0.02] p-8 text-center">
              <Search className="mx-auto h-8 w-8 text-muted-foreground" />
              <p className="mt-2 text-sm font-medium">未找到匹配的技能</p>
              <p className="mt-1 text-xs text-muted-foreground">
                尝试其他关键词或切换来源筛选
              </p>
            </div>
          )}

          {/* Grouped Results */}
          <div className="space-y-6">
            {Object.entries(groupedBySource).map(([source, skills]) => {
              const config = SOURCE_CONFIG[source] || {
                label: source,
                icon: Globe,
                color: 'text-gray-700',
              }
              const Icon = config.icon

              return (
                <div key={source} className="space-y-3">
                  <div className="flex items-center gap-2">
                    <Icon className={cn('h-4 w-4', config.color)} />
                    <h4 className="font-medium">{config.label}</h4>
                    <Badge variant="secondary" className="text-xs">
                      {skills.length}
                    </Badge>
                  </div>

                  <div className="grid gap-3 md:grid-cols-2 lg:grid-cols-3">
                    {skills.map((skill) => {
                      const isInstalled = installedSkillNames.has(skill.name.toLowerCase())
                      const trustColors = TRUST_COLORS[skill.trust_level] || TRUST_COLORS.community

                      return (
                        <article
                          key={`${skill.source}:${skill.identifier}`}
                          className={cn(
                            'rounded-[16px] border border-black/8 bg-white/90 p-4 transition-shadow',
                            'hover:shadow-md cursor-pointer',
                            isInstalled && 'opacity-70'
                          )}
                          onClick={() => setSelectedSkill(skill)}
                        >
                          <div className="flex items-start justify-between gap-2">
                            <div className="min-w-0 flex-1">
                              <div className="flex items-center gap-2">
                                <span className="truncate text-[15px] font-semibold">
                                  {skill.name}
                                </span>
                                {isInstalled && (
                                  <CheckCircle className="h-4 w-4 shrink-0 text-emerald-600" />
                                )}
                              </div>
                              <p className="mt-1 line-clamp-2 text-sm text-black/65">
                                {skill.description}
                              </p>
                            </div>
                          </div>

                          <div className="mt-3 flex flex-wrap gap-1.5">
                            <span
                              className={cn(
                                'rounded-full px-2 py-0.5 text-[10px] font-medium',
                                trustColors.bg,
                                trustColors.text
                              )}
                            >
                              {TRUST_LABELS[skill.trust_level]}
                            </span>
                            {skill.tags.slice(0, 3).map((tag) => (
                              <Badge
                                key={tag}
                                variant="outline"
                                className="rounded-full px-2 py-0.5 text-[10px]"
                              >
                                {tag}
                              </Badge>
                            ))}
                          </div>

                          <div className="mt-3 flex items-center justify-between">
                            <span className="truncate text-xs text-muted-foreground">
                              {skill.repo || skill.source}
                            </span>
                            <Button
                              type="button"
                              size="sm"
                              variant={isInstalled ? 'outline' : 'default'}
                              className="h-7 rounded-full px-3 text-xs"
                              onClick={(e) => {
                                e.stopPropagation()
                                onInstall(skill)
                              }}
                              disabled={isInstalled}
                            >
                              {isInstalled ? '已安装' : '安装'}
                            </Button>
                          </div>
                        </article>
                      )
                    })}
                  </div>
                </div>
              )
            })}
          </div>
        </>
      )}

      {/* Skill Detail Modal */}
      {selectedSkill && (
        <SkillDetailModal
          skill={selectedSkill}
          isInstalled={installedSkillNames.has(selectedSkill.name.toLowerCase())}
          onClose={() => setSelectedSkill(null)}
          onInstall={() => {
            onInstall(selectedSkill)
            setSelectedSkill(null)
          }}
        />
      )}
    </div>
  )
}

function SkillDetailModal({
  skill,
  isInstalled,
  onClose,
  onInstall,
}: {
  skill: HubSearchResult
  isInstalled: boolean
  onClose: () => void
  onInstall: () => void
}) {
  const trustColors = TRUST_COLORS[skill.trust_level] || TRUST_COLORS.community
  const config = SOURCE_CONFIG[skill.source] || { label: skill.source, icon: Globe, color: 'text-gray-700' }
  const Icon = config.icon

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
      onClick={onClose}
    >
      <div
        className="w-[min(90vw,600px)] rounded-[24px] border border-black/10 bg-white p-6 shadow-xl"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-start justify-between">
          <div>
            <div className="flex items-center gap-2">
              <Icon className={cn('h-5 w-5', config.color)} />
              <h3 className="text-xl font-semibold">{skill.name}</h3>
              {isInstalled && (
                <Badge className="bg-emerald-100 text-emerald-700">已安装</Badge>
              )}
            </div>
            <p className="mt-1 text-sm text-muted-foreground">{skill.source}</p>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="rounded-full p-1 hover:bg-black/[0.04]"
          >
            <XCircle className="h-5 w-5" />
          </button>
        </div>

        <div className="mt-4 space-y-4">
          <div>
            <h4 className="text-sm font-medium">描述</h4>
            <p className="mt-1 text-sm text-muted-foreground">{skill.description}</p>
          </div>

          <div className="flex flex-wrap gap-2">
            <span
              className={cn(
                'rounded-full px-2.5 py-1 text-xs font-medium',
                trustColors.bg,
                trustColors.text
              )}
            >
              {TRUST_LABELS[skill.trust_level]}
            </span>
            {skill.tags.map((tag) => (
              <Badge key={tag} variant="outline" className="rounded-full">
                {tag}
              </Badge>
            ))}
          </div>

          {skill.repo && (
            <div>
              <h4 className="text-sm font-medium">仓库</h4>
              <p className="mt-1 text-sm font-mono text-muted-foreground">{skill.repo}</p>
            </div>
          )}

          {skill.path && (
            <div>
              <h4 className="text-sm font-medium">路径</h4>
              <p className="mt-1 text-sm font-mono text-muted-foreground">{skill.path}</p>
            </div>
          )}

          <div>
            <h4 className="text-sm font-medium">标识符</h4>
            <p className="mt-1 text-sm font-mono text-muted-foreground">{skill.identifier}</p>
          </div>
        </div>

        <div className="mt-6 flex justify-end gap-3">
          <Button type="button" variant="outline" onClick={onClose}>
            关闭
          </Button>
          <Button onClick={onInstall} disabled={isInstalled}>
            {isInstalled ? '已安装' : '安装'}
          </Button>
        </div>
      </div>
    </div>
  )
}

export default SkillsHubView
