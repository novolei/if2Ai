import { useEffect, useMemo, useState } from 'react'
import { toast } from 'sonner'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Switch } from '@/components/ui/switch'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import {
  fetchSkillsMarketAudits,
  hubBrowse,
  hubInstall,
  hubSearch,
  installSkillFromDistribution,
  type HubSkillResult,
  type SkillInfo,
} from '@/lib/tauri'
import { cn } from '@/lib/utils'
import {
  BotMessageSquare,
  CheckCircle2,
  Code2,
  FileText,
  Filter,
  Globe,
  MoreHorizontal,
  Plus,
  Search,
  ShieldAlert,
  Star,
  XCircle,
} from 'lucide-react'
import { SettingsSurface } from '../components/SettingsSurface'

type SkillCategoryId = 'all' | 'recent' | 'docs' | 'search' | 'coding' | 'web' | 'collab' | 'media'
type SkillSourceFilter = 'all' | 'builtin' | 'user'
type SkillsTab = 'installed' | 'market'
type MarketSource = 'skills-sh' | 'github'

interface SkillsSettingsPageProps {
  skills: SkillInfo[]
  loading: boolean
  error: string | null
  onRefresh: () => void
  onToggleSkill: (skill: SkillInfo, enabled: boolean) => void
  onStartConversationCreate: () => void
  onReviewSkill: (skill: SkillInfo) => void
  onApproveSkill: (skill: SkillInfo) => void
  onRollbackSkill: (skill: SkillInfo) => void
}

interface SkillCategoryMeta {
  id: SkillCategoryId
  label: string
}

interface MarketSkill {
  skill: string
  repo: string
  gen: string
  socketAlerts: string
  snykRisk: string
}

interface MarketCachePayload {
  updatedAt: number
  data: MarketSkill[]
}

const CATEGORY_TABS: SkillCategoryMeta[] = [
  { id: 'all', label: '全部' },
  { id: 'recent', label: '近期使用' },
  { id: 'docs', label: '文档内容' },
  { id: 'search', label: '搜索研究' },
  { id: 'coding', label: '代码开发' },
  { id: 'web', label: '网页自动化' },
  { id: 'collab', label: '协作通讯' },
  { id: 'media', label: '多媒体AI' },
]

const MARKET_SEED: MarketSkill[] = [
  { skill: 'find-skills', repo: 'vercel-labs/skills', gen: 'Safe', socketAlerts: '0 alerts', snykRisk: 'Med Risk' },
  { skill: 'frontend-design', repo: 'anthropics/skills', gen: 'Safe', socketAlerts: '0 alerts', snykRisk: 'Low Risk' },
  { skill: 'web-design-guidelines', repo: 'vercel-labs/agent-skills', gen: 'Safe', socketAlerts: '0 alerts', snykRisk: 'Med Risk' },
  { skill: 'microsoft-foundry', repo: 'microsoft/azure-skills', gen: 'Safe', socketAlerts: '0 alerts', snykRisk: 'Med Risk' },
  { skill: 'azure-ai', repo: 'microsoft/azure-skills', gen: 'Safe', socketAlerts: '0 alerts', snykRisk: 'Low Risk' },
  { skill: 'azure-deploy', repo: 'microsoft/azure-skills', gen: 'Safe', socketAlerts: '0 alerts', snykRisk: 'Low Risk' },
  { skill: 'azure-prepare', repo: 'microsoft/azure-skills', gen: 'Safe', socketAlerts: '0 alerts', snykRisk: 'Critical' },
  { skill: 'azure-validate', repo: 'microsoft/azure-skills', gen: 'Safe', socketAlerts: '0 alerts', snykRisk: 'Critical' },
  { skill: 'azure-rbac', repo: 'microsoft/azure-skills', gen: 'Safe', socketAlerts: '0 alerts', snykRisk: 'Low Risk' },
  { skill: 'skill-creator', repo: 'anthropics/skills', gen: 'Safe', socketAlerts: '0 alerts', snykRisk: 'Low Risk' },
]
const MARKET_CACHE_KEY = 'if2ai.skills-market.cache.v1'
const MARKET_CACHE_TTL_MS = 1000 * 60 * 60 * 12

export function SkillsSettingsPage({
  skills,
  loading,
  error,
  onRefresh,
  onToggleSkill,
  onStartConversationCreate,
  onReviewSkill,
  onApproveSkill,
  onRollbackSkill,
}: SkillsSettingsPageProps) {
  const [activeTab, setActiveTab] = useState<SkillsTab>('installed')
  const [query, setQuery] = useState('')
  const [activeCategory, setActiveCategory] = useState<SkillCategoryId>('all')
  const [sourceFilter, setSourceFilter] = useState<SkillSourceFilter>('all')
  const [githubDialogOpen, setGithubDialogOpen] = useState(false)
  const [githubImportUrl, setGithubImportUrl] = useState('')
  const [githubImportName, setGithubImportName] = useState('')
  const [marketLoading, setMarketLoading] = useState(false)
  const [marketError, setMarketError] = useState<string | null>(null)
  const [marketSkills, setMarketSkills] = useState<MarketSkill[]>(MARKET_SEED)
  const [marketQuery, setMarketQuery] = useState('')
  const [marketOnlyInstalled, setMarketOnlyInstalled] = useState(false)
  const [marketPage, setMarketPage] = useState(1)
  const [marketPageSize, setMarketPageSize] = useState(10)
  const [marketInstallOpen, setMarketInstallOpen] = useState(false)
  const [marketInstallName, setMarketInstallName] = useState('')
  const [marketInstallUrl, setMarketInstallUrl] = useState('')
  const [marketInstallChannel, setMarketInstallChannel] = useState<'stable' | 'canary'>('stable')
  const [marketInstallChecksum, setMarketInstallChecksum] = useState('')
  const [marketInstallSignature, setMarketInstallSignature] = useState('')
  const [marketNotice, setMarketNotice] = useState<string | null>(null)
  const [marketSource, setMarketSource] = useState<MarketSource>('skills-sh')
  const [githubSkills, setGithubSkills] = useState<HubSkillResult[]>([])
  const [githubLoading, setGithubLoading] = useState(false)
  const [githubError, setGithubError] = useState<string | null>(null)
  const [githubSearchQuery, setGithubSearchQuery] = useState('')

  const installedSkillKeys = useMemo(() => {
    const keys = new Set<string>()
    for (const skill of skills) {
      keys.add(skill.name.toLowerCase())
    }
    return keys
  }, [skills])

  const categoryCounter = useMemo(() => {
    const counter: Record<SkillCategoryId, number> = {
      all: skills.length,
      recent: 0,
      docs: 0,
      search: 0,
      coding: 0,
      web: 0,
      collab: 0,
      media: 0,
    }
    for (const skill of skills) {
      if (skill.enabled) counter.recent += 1
      counter[classifySkill(skill)] += 1
    }
    return counter
  }, [skills])

  const filteredSkills = useMemo(() => {
    const loweredQuery = query.trim().toLowerCase()
    return skills.filter((skill) => {
      if (sourceFilter === 'builtin' && skill.source !== 'builtin') return false
      if (sourceFilter === 'user' && !(skill.source === 'user' || skill.source === 'workspace')) return false
      if (activeCategory === 'recent' && !skill.enabled) return false
      if (activeCategory !== 'all' && activeCategory !== 'recent' && classifySkill(skill) !== activeCategory) {
        return false
      }
      if (!loweredQuery) return true
      return (
        skill.name.toLowerCase().includes(loweredQuery) ||
        skill.path.toLowerCase().includes(loweredQuery) ||
        skill.description.toLowerCase().includes(loweredQuery)
      )
    })
  }, [skills, query, sourceFilter, activeCategory])

  const filteredMarket = useMemo(() => {
    const lowered = marketQuery.trim().toLowerCase()
    const rows = marketSkills.filter((item) => {
      const installed = installedSkillKeys.has(item.skill.toLowerCase())
      if (marketOnlyInstalled && !installed) return false
      if (!lowered) return true
      return (
        item.skill.toLowerCase().includes(lowered) ||
        item.repo.toLowerCase().includes(lowered) ||
        item.snykRisk.toLowerCase().includes(lowered)
      )
    })
    return rows
  }, [marketSkills, marketQuery, marketOnlyInstalled, installedSkillKeys])

  const pagedMarket = useMemo(() => {
    const start = (marketPage - 1) * marketPageSize
    return filteredMarket.slice(start, start + marketPageSize)
  }, [filteredMarket, marketPage, marketPageSize])

  const maxMarketPage = Math.max(1, Math.ceil(filteredMarket.length / marketPageSize))

  useEffect(() => {
    if (marketPage > maxMarketPage) {
      setMarketPage(maxMarketPage)
    }
  }, [marketPage, maxMarketPage])

  useEffect(() => {
    const cached = loadMarketCache()
    if (cached && cached.data.length > 0) {
      setMarketSkills(cached.data)
      setMarketNotice(`已加载本地缓存（${formatCacheTime(cached.updatedAt)}）`)
    }
  }, [])

  useEffect(() => {
    if (activeTab === 'market') {
      void refreshMarketFromAudits(false)
    }
  }, [activeTab])

  useEffect(() => {
    if (activeTab === 'market' && marketSource === 'github' && githubSkills.length === 0) {
      void loadGithubSkills()
    }
  }, [activeTab, marketSource])

  const refreshMarketFromAudits = async (forceRefresh: boolean) => {
    const cached = loadMarketCache()
    if (
      !forceRefresh &&
      cached &&
      cached.data.length > 0 &&
      Date.now() - cached.updatedAt < MARKET_CACHE_TTL_MS
    ) {
      setMarketSkills(cached.data)
      setMarketError(null)
      toast.info('已使用本地缓存', { description: `更新于 ${formatCacheTime(cached.updatedAt)}` })
      return
    }

    setMarketLoading(true)
    setMarketError(null)
    try {
      const parsed = await fetchSkillsMarketAudits()
      if (parsed.length > 0) {
        setMarketSkills(parsed)
        saveMarketCache({ updatedAt: Date.now(), data: parsed })
        toast.success(`已更新 ${parsed.length} 条技能审计数据`)
      } else {
        throw new Error('解析失败：未提取到有效技能列表')
      }
    } catch (error) {
      const fallback = loadMarketCache()
      if (fallback && fallback.data.length > 0) {
        setMarketSkills(fallback.data)
        setMarketError(null)
        toast.warning('网络异常，已使用本地缓存', { description: `更新于 ${formatCacheTime(fallback.updatedAt)}` })
      } else {
        setMarketSkills(MARKET_SEED)
        toast.error('网络异常，已使用内置数据', { description: String(error) })
      }
    } finally {
      setMarketLoading(false)
    }
  }

  const handleGithubImportPlaceholder = async () => {
    if (!githubImportUrl.trim()) return
    const derived = githubImportName.trim() || deriveSkillNameFromGithub(githubImportUrl) || 'new-skill'
    toast.info('GitHub 导入已记录', { description: `技能「${derived}」来源 URL=${githubImportUrl}` })
    setGithubDialogOpen(false)
  }

  const loadGithubSkills = async (query = '') => {
    setGithubLoading(true)
    setGithubError(null)
    try {
      const result = query.trim()
        ? await hubSearch(query.trim(), 'github', 40)
        : await hubBrowse('github', 40)
      setGithubSkills(result.data ?? [])
      if ((result.data ?? []).length === 0) {
        toast.warning('未找到匹配技能', { description: 'GitHub API 匿名访问有速率限制，稍后重试' })
      } else {
        toast.success(`已加载 ${result.data?.length ?? 0} 个 GitHub 技能`)
      }
    } catch (error) {
      toast.error('GitHub 拉取失败', { description: String(error) })
    } finally {
      setGithubLoading(false)
    }
  }

  const handleInstallFromMarket = async (item: MarketSkill) => {
    try {
      // source_id for skills.sh hub adapter is "skills-sh" (not "skills.sh")
      const result = await hubInstall('skills-sh', `${item.repo}/${item.skill}`)
      if (result.success) {
        toast.success(`已安装「${item.skill}」`, { description: result.message })
        onRefresh()
      } else {
        toast.error(`安装失败：${item.skill}`, { description: result.message })
      }
    } catch (error) {
      toast.error(`安装出错：${item.skill}`, { description: String(error) })
    }
  }

  const handleInstallGithubSkill = async (skill: HubSkillResult) => {
    try {
      const result = await hubInstall('github', skill.identifier)
      if (result.success) {
        toast.success(`已安装「${skill.name}」`, { description: result.message })
        onRefresh()
      } else {
        toast.error(`安装失败：${skill.name}`, { description: result.message })
      }
    } catch (error) {
      toast.error(`安装出错：${skill.name}`, { description: String(error) })
    }
  }

  const handleManualInstallSubmit = async () => {
    if (!marketInstallName.trim()) return
    try {
      await installSkillFromDistribution({
        skillName: marketInstallName.trim(),
        url: marketInstallUrl.trim(),
        channel: marketInstallChannel,
        checksum: marketInstallChecksum.trim(),
        signature: marketInstallSignature.trim(),
      })
      toast.success(`已安装「${marketInstallName.trim()}」`)
      setMarketInstallOpen(false)
      onRefresh()
    } catch (error) {
      toast.error('安装失败', { description: String(error) })
    }
  }

  const attemptReview = (skill: SkillInfo, allowed: boolean) => {
    if (!allowed) {
      toast.warning(`「${skill.name}」暂不可 Review`, { description: `当前状态为 ${skill.review_status}` })
      return
    }
    toast.loading('正在 Review…', { id: `review-${skill.path}` })
    onReviewSkill(skill)
  }

  const attemptApprove = (skill: SkillInfo, allowed: boolean) => {
    if (!allowed) {
      toast.warning(`「${skill.name}」暂不可批准`, { description: `当前状态为 ${skill.review_status}` })
      return
    }
    toast.success(`「${skill.name}」已批准为 Active`)
    onApproveSkill(skill)
  }

  const attemptRollback = (skill: SkillInfo, allowed: boolean) => {
    if (!allowed) {
      toast.warning(`「${skill.name}」暂不可回滚`, { description: `当前状态为 ${skill.review_status}` })
      return
    }
    toast.info(`「${skill.name}」已回滚到 Quarantine`)
    onRollbackSkill(skill)
  }

  return (
    <div className="flex flex-col gap-4">
      {/* Page-level error only */}
      {error && <ErrorBanner text={`技能加载失败：${error}`} />}

      <Tabs value={activeTab} onValueChange={(value) => setActiveTab(value as SkillsTab)}>
        <TabsList className="h-9 rounded-full bg-black/[0.04] p-1">
          <TabsTrigger value="installed" className="rounded-full px-4 py-1 text-[13px]">
            我的技能
          </TabsTrigger>
          <TabsTrigger value="market" className="rounded-full px-4 py-1 text-[13px]">
            Skills Market
          </TabsTrigger>
        </TabsList>

        <TabsContent value="installed" className="space-y-4">
          {/* Search + Add */}
          <div className="flex items-center gap-3">
            <div className="relative flex-1">
              <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
              <input
                value={query}
                onChange={(event) => setQuery(event.target.value)}
                placeholder="搜索已经安装的技能"
                className="h-10 w-full rounded-[999px] border border-black/10 bg-white/84 pl-10 pr-3 text-[13px] outline-none ring-offset-background transition focus:ring-1 focus:ring-ring"
              />
            </div>
            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <Button type="button" variant="outline" className="h-10 rounded-[999px] border border-black/12 bg-white/84 px-4 text-[13px]">
                  <Plus className="mr-1 h-4 w-4" />
                  添加技能
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent className="w-[280px] rounded-[6px] p-3" align="end">
                <button
                  type="button"
                  onClick={onStartConversationCreate}
                  className="w-full rounded-[6px] bg-black/[0.04] px-4 py-3 text-left transition hover:bg-black/[0.06]"
                >
                  <div className="flex items-center gap-3">
                    <BotMessageSquare className="h-5 w-5 text-black/75" />
                    <div>
                      <div className="text-[17px] font-medium">通过对话创建</div>
                      <div className="text-[13px] text-muted-foreground">描述你的需求，AI 帮你生成</div>
                    </div>
                  </div>
                </button>
                <button
                  type="button"
                  onClick={() => {
                    setGithubImportUrl('')
                    setGithubImportName('')
                    setGithubDialogOpen(true)
                  }}
                  className="mt-1 w-full rounded-[6px] px-4 py-3 text-left transition hover:bg-black/[0.04]"
                >
                  <div className="flex items-center gap-3">
                    <Globe className="h-5 w-5 text-black/85" />
                    <div>
                      <div className="text-[17px] font-medium">从 GitHub 导入</div>
                      <div className="text-[13px] text-muted-foreground">粘贴一个仓库连接以开始</div>
                    </div>
                  </div>
                </button>
              </DropdownMenuContent>
            </DropdownMenu>
          </div>

          {/* Category tabs + Source filter */}
          <div className="flex flex-wrap items-center justify-between gap-3">
            <div className="flex flex-wrap items-center gap-1.5">
              {CATEGORY_TABS.map((tab) => {
                const active = tab.id === activeCategory
                return (
                  <button
                    key={tab.id}
                    type="button"
                    onClick={() => setActiveCategory(tab.id)}
                    className={cn(
                      'rounded-full px-3 py-1.5 text-[13px] transition',
                      active ? 'bg-black/90 text-white' : 'bg-black/[0.04] text-foreground hover:bg-black/[0.08]'
                    )}
                  >
                    {tab.label}
                    <span className="ml-1.5 text-[12px] opacity-80">{categoryCounter[tab.id]}</span>
                  </button>
                )
              })}
            </div>

            <div className="flex items-center gap-2">
              <Filter className="h-4 w-4 text-muted-foreground" />
              <Select value={sourceFilter} onValueChange={(value) => setSourceFilter(value as SkillSourceFilter)}>
                <SelectTrigger className="w-[158px] rounded-[999px]">
                  <SelectValue placeholder="全部来源" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="all">全部来源</SelectItem>
                  <SelectItem value="builtin">内置技能</SelectItem>
                  <SelectItem value="user">用户/工作区技能</SelectItem>
                </SelectContent>
              </Select>
            </div>
          </div>

          {loading && <InfoBanner text="正在加载技能列表..." tone="neutral" />}

          {/* Agent Proposal Banner */}
          {!loading && <AgentProposalBanner
            skills={skills}
            onReview={onReviewSkill}
            onApprove={onApproveSkill}
            onRollback={onRollbackSkill}
          />}

          {/* Skill cards */}
          {!loading && (
            <div className="grid gap-2.5 md:grid-cols-2">
              {filteredSkills.map((skill) => {
                const cardMeta = visualMetaForSkill(skill)
                const canReview =
                  skill.source !== 'builtin' &&
                  (skill.review_status === 'draft' || skill.review_status === 'quarantine')
                const canApprove = skill.source !== 'builtin' && skill.review_status === 'review_passed'
                const canRollback =
                  skill.source !== 'builtin' &&
                  (skill.review_status === 'active' || skill.review_status === 'review_passed')

                return (
                  <article
                    key={skill.path}
                    className="group flex flex-col rounded-xl border border-border/50 bg-surface-raised p-4 shadow-token-xs transition-all hover:shadow-token-sm hover:border-primary/20"
                  >
                    <div className="flex items-start gap-3.5">
                      <div className={cn('flex h-10 w-10 shrink-0 items-center justify-center rounded-lg', cardMeta.iconBgClass)}>
                        <cardMeta.icon className={cn('h-5 w-5', cardMeta.iconClass)} />
                      </div>
                      <div className="min-w-0 flex-1">
                        <div className="flex items-center gap-2">
                          <span className="text-[14px] font-semibold tracking-tight">{skill.name}</span>
                          <StatusPill status={skill.review_status} />
                        </div>
                        <p className="mt-0.5 line-clamp-1 text-[12px] text-muted-foreground">
                          {skill.description || fallbackDescription(skill)}
                        </p>
                      </div>
                      <Switch
                        checked={skill.enabled}
                        onCheckedChange={(enabled) => onToggleSkill(skill, enabled)}
                        aria-label={`toggle ${skill.name}`}
                      />
                    </div>

                    <div className="mt-3 flex flex-wrap gap-1.5">
                      <Badge variant="outline" className="rounded-full px-2 py-0 text-[10px]">
                        {sourceLabel(skill.source)}
                      </Badge>
                      <Badge variant="outline" className="rounded-full px-2 py-0 text-[10px]">
                        {categoryLabel(classifySkill(skill))}
                      </Badge>
                      {skill.shadowed_by && (
                        <Badge variant="outline" className="rounded-full px-2 py-0 text-[10px] text-amber-600 border-amber-200">
                          被 {skill.shadowed_by} 遮挡
                        </Badge>
                      )}
                    </div>

                    <div className="mt-3 flex items-center justify-between border-t border-border/50 pt-3">
                      <span className="truncate text-[11px] font-mono text-muted-foreground">{skill.path}</span>
                      {skill.source !== 'builtin' && (
                        <DropdownMenu>
                          <DropdownMenuTrigger asChild>
                            <Button type="button" variant="ghost" size="icon" className="h-7 w-7 rounded-full text-muted-foreground hover:bg-black/[0.04]">
                              <MoreHorizontal className="h-3.5 w-3.5" />
                            </Button>
                          </DropdownMenuTrigger>
                          <DropdownMenuContent align="end">
                            <DropdownMenuItem onClick={() => attemptReview(skill, canReview)}>
                              Review 技能
                            </DropdownMenuItem>
                            <DropdownMenuItem onClick={() => attemptApprove(skill, canApprove)}>
                              批准为 Active
                            </DropdownMenuItem>
                            <DropdownMenuItem onClick={() => attemptRollback(skill, canRollback)}>
                              回滚到 Quarantine
                            </DropdownMenuItem>
                          </DropdownMenuContent>
                        </DropdownMenu>
                      )}
                    </div>
                  </article>
                )
              })}
            </div>
          )}

          {/* Empty state */}
          {!loading && filteredSkills.length === 0 && (
            <div className="rounded-xl border border-border/50 bg-black/[0.02] p-8 text-center">
              <div className="text-sm font-medium">没有匹配的技能</div>
              <p className="mt-1 text-xs text-muted-foreground">试试清空关键词，或者切换来源和分类筛选。</p>
              <Button
                type="button"
                variant="outline"
                className="mt-3 h-8 rounded-full px-3 text-xs"
                onClick={() => {
                  setQuery('')
                  setActiveCategory('all')
                  setSourceFilter('all')
                }}
              >
                重置筛选
              </Button>
            </div>
          )}
        </TabsContent>

        <TabsContent value="market" className="space-y-4">
          {/* Market header */}
          <div className="flex items-center justify-between">
            <div>
              <h4 className="text-[15px] font-semibold tracking-tight">Skills Market</h4>
              <p className="text-[12px] text-muted-foreground">
                发现并一键安装社区技能，安装后经安全扫描写入隔离区。
              </p>
            </div>
            <div className="flex items-center gap-2">
              {marketSource === 'skills-sh' && (
                <Button type="button" variant="outline" onClick={() => void refreshMarketFromAudits(true)} className="h-8 rounded-full px-3 text-xs">
                  刷新
                </Button>
              )}
              {marketSource === 'github' && (
                <Button type="button" variant="outline" onClick={() => void loadGithubSkills(githubSearchQuery)} className="h-8 rounded-full px-3 text-xs">
                  刷新
                </Button>
              )}
            </div>
          </div>

          {/* Source selector */}
          <div className="flex gap-1 rounded-full border border-black/10 bg-black/[0.03] p-1 w-fit">
            {([
              { id: 'skills-sh' as MarketSource, label: 'skills.sh', hint: '社区审计数据' },
              { id: 'github' as MarketSource, label: 'GitHub 精选', hint: '官方 DEFAULT_TAPS' },
            ] as const).map((src) => (
              <button
                key={src.id}
                type="button"
                onClick={() => setMarketSource(src.id)}
                className={cn(
                  'rounded-full px-4 py-1 text-xs font-medium transition-all',
                  marketSource === src.id
                    ? 'bg-white shadow-sm text-black'
                    : 'text-black/50 hover:text-black/70'
                )}
              >
                {src.label}
              </button>
            ))}
          </div>

          {/* skills.sh panel */}
          {marketSource === 'skills-sh' && (
            <div className="space-y-4">
              <div className="flex items-center gap-3">
                <div className="relative flex-1">
                  <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
                  <input
                    value={marketQuery}
                    onChange={(event) => {
                      setMarketQuery(event.target.value)
                      setMarketPage(1)
                    }}
                    placeholder="搜索技能（例如：azure, git, creator）"
                    className="h-9 w-full rounded-[999px] border border-black/10 bg-white/84 pl-10 pr-3 text-[13px] outline-none ring-offset-background transition focus:ring-1 focus:ring-ring"
                  />
                </div>
                <div className="flex items-center gap-2 rounded-full border border-black/10 bg-white/84 px-3 py-1">
                  <span className="text-xs text-muted-foreground">仅看已安装</span>
                  <Switch checked={marketOnlyInstalled} onCheckedChange={setMarketOnlyInstalled} aria-label="only-installed" />
                </div>
              </div>

              <div className="grid gap-2.5 md:grid-cols-4">
                <StatCard label="匹配结果" value={String(filteredMarket.length)} hint="skills.sh 审计列表" />
                <StatCard label="已安装" value={String(Array.from(installedSkillKeys).length)} hint="已激活技能数" />
                <StatCard label="已验证" value={String(filteredMarket.filter((item) => item.gen.toLowerCase() === 'safe').length)} hint="Gen Agent Trust" />
                <StatCard label="高风险" value={String(filteredMarket.filter((item) => item.snykRisk.toLowerCase().includes('critical') || item.snykRisk.toLowerCase().includes('high')).length)} hint="Snyk + Socket 维度" />
              </div>

              {marketNotice && <InfoBanner text={marketNotice} tone="neutral" />}
              {marketError && <ErrorBanner text={marketError} />}
              {marketLoading && <InfoBanner text="正在从 skills.sh/audits 拉取安全审计数据…" tone="neutral" />}

              <div className="flex items-center justify-between text-xs text-muted-foreground">
                <div>第 {marketPage} 页 / 共 {maxMarketPage} 页</div>
                <div className="flex items-center gap-2">
                  <span>每页</span>
                  <Select value={String(marketPageSize)} onValueChange={(value) => { setMarketPageSize(Number(value)); setMarketPage(1) }}>
                    <SelectTrigger className="h-8 w-[84px] rounded-full"><SelectValue /></SelectTrigger>
                    <SelectContent>
                      <SelectItem value="10">10</SelectItem>
                      <SelectItem value="20">20</SelectItem>
                      <SelectItem value="50">50</SelectItem>
                    </SelectContent>
                  </Select>
                  <Button type="button" variant="outline" size="sm" className="h-8 rounded-full px-3 text-xs" disabled={marketPage <= 1} onClick={() => setMarketPage((v) => Math.max(1, v - 1))}>上一页</Button>
                  <Button type="button" variant="outline" size="sm" className="h-8 rounded-full px-3 text-xs" disabled={marketPage >= maxMarketPage} onClick={() => setMarketPage((v) => Math.min(maxMarketPage, v + 1))}>下一页</Button>
                </div>
              </div>

              <div className="grid gap-2.5 md:grid-cols-2">
                {pagedMarket.map((item) => {
                  const installed = installedSkillKeys.has(item.skill.toLowerCase())
                  const isHighRisk = item.snykRisk.toLowerCase().includes('critical') || item.snykRisk.toLowerCase().includes('high')
                  return (
                    <article key={`${item.skill}:${item.repo}`} className="rounded-xl border border-border/50 bg-surface-raised p-4 shadow-token-xs transition-all hover:shadow-token-sm hover:border-primary/20">
                      <div className="flex items-start justify-between gap-2">
                        <div className="min-w-0">
                          <div className="truncate text-[14px] font-semibold tracking-tight">{item.skill}</div>
                          <p className="truncate text-[11px] text-muted-foreground">{item.repo}</p>
                        </div>
                        {installed && <Badge className="shrink-0 rounded-full bg-emerald-50 text-emerald-600 text-[10px] border border-emerald-200">已安装</Badge>}
                      </div>
                      <div className="mt-2 flex flex-wrap gap-1.5">
                        <Badge variant="outline" className="rounded-full px-2 text-[10px]">{item.gen}</Badge>
                        <Badge variant="outline" className="rounded-full px-2 text-[10px]">
                          <Star className="mr-1 h-2.5 w-2.5" />{item.socketAlerts}
                        </Badge>
                        <Badge variant="outline" className={cn('rounded-full px-2 text-[10px]', isHighRisk ? 'border-red-200 text-red-600' : 'border-amber-200 text-amber-600')}>
                          <ShieldAlert className="mr-1 h-2.5 w-2.5" />{item.snykRisk}
                        </Badge>
                      </div>
                      <div className="mt-3 flex items-center gap-2">
                        <Button
                          type="button"
                          size="sm"
                          disabled={installed}
                          className="h-8 rounded-full px-3 text-xs"
                          onClick={() => void handleInstallFromMarket(item)}
                        >
                          {installed ? '已安装' : '安装'}
                        </Button>
                        <span className="text-[10px] text-muted-foreground">via skills-sh</span>
                      </div>
                    </article>
                  )
                })}
              </div>
            </div>
          )}

          {/* GitHub panel */}
          {marketSource === 'github' && (
            <div className="space-y-4">
              <div className="rounded-xl border border-blue-100 bg-blue-50 px-4 py-2.5 text-xs text-blue-700">
                来源：DEFAULT_TAPS（openai/skills · anthropics/skills · VoltAgent/awesome-agent-skills · garrytan/gstack）
                · 匿名访问 GitHub API，有速率限制。
              </div>

              <div className="relative">
                <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
                <input
                  value={githubSearchQuery}
                  onChange={(event) => setGithubSearchQuery(event.target.value)}
                  onKeyDown={(event) => { if (event.key === 'Enter') void loadGithubSkills(githubSearchQuery) }}
                  placeholder="输入关键词后回车搜索 GitHub 技能"
                  className="h-9 w-full rounded-[999px] border border-black/10 bg-white/84 pl-10 pr-3 text-[13px] outline-none ring-offset-background transition focus:ring-1 focus:ring-ring"
                />
              </div>

              {githubLoading && <InfoBanner text="正在通过 GitHub API 拉取技能列表…" tone="neutral" />}
              {githubError && <ErrorBanner text={githubError} />}

              {githubSkills.length > 0 && (
                <div className="grid gap-2.5 md:grid-cols-2">
                  {githubSkills.map((skill) => {
                    const installed = installedSkillKeys.has(skill.name.toLowerCase())
                    const isTrusted = skill.trust_level === 'Trusted'
                    return (
                      <article key={skill.identifier} className="rounded-xl border border-border/50 bg-surface-raised p-4 shadow-token-xs transition-all hover:shadow-token-sm hover:border-primary/20">
                        <div className="flex items-start justify-between gap-2">
                          <div className="min-w-0">
                            <div className="truncate text-[14px] font-semibold tracking-tight">{skill.name}</div>
                            <p className="truncate text-[11px] text-muted-foreground">{skill.identifier}</p>
                          </div>
                          <div className="flex shrink-0 gap-1">
                            {installed && <Badge className="rounded-full bg-emerald-50 text-emerald-600 text-[10px] border border-emerald-200">已安装</Badge>}
                            {isTrusted && <Badge className="rounded-full bg-blue-50 text-blue-600 text-[10px] border border-blue-200">官方</Badge>}
                          </div>
                        </div>
                        {skill.description && (
                          <p className="mt-1 text-[12px] text-muted-foreground line-clamp-2">{skill.description}</p>
                        )}
                        {skill.tags.length > 0 && (
                          <div className="mt-2 flex flex-wrap gap-1">
                            {skill.tags.slice(0, 4).map((tag) => (
                              <span key={tag} className="rounded-full bg-black/[0.05] px-2 py-0.5 text-[10px] text-black/60">{tag}</span>
                            ))}
                          </div>
                        )}
                        <div className="mt-3 flex items-center gap-2">
                          <Button
                            type="button"
                            size="sm"
                            disabled={installed}
                            className="h-8 rounded-full px-3 text-xs"
                            onClick={() => void handleInstallGithubSkill(skill)}
                          >
                            {installed ? '已安装' : '安装'}
                          </Button>
                          <span className="text-[10px] text-muted-foreground">via github</span>
                        </div>
                      </article>
                    )
                  })}
                </div>
              )}

              {!githubLoading && githubSkills.length === 0 && !githubError && (
                <div className="rounded-xl border border-border/50 bg-black/[0.02] p-8 text-center">
                  <p className="text-sm text-muted-foreground">点击刷新或输入关键词搜索 GitHub 技能</p>
                </div>
              )}
            </div>
          )}
        </TabsContent>
      </Tabs>

      {/* GitHub Import Dialog */}
      <Dialog open={githubDialogOpen} onOpenChange={setGithubDialogOpen}>
        <DialogContent className="w-[min(92vw,36rem)] rounded-[6px] border border-black/10 bg-[#f7f7f8] p-6">
          <DialogHeader>
            <DialogTitle className="text-[42px] text-3xl font-semibold tracking-tight">从 GitHub 导入</DialogTitle>
            <DialogDescription className="text-[30px] text-xl text-black/45">
              直接从公开的 GitHub 仓库中导入技能（当前为占位流程）
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-4">
            <div className="flex items-center gap-4">
              <label className="w-14 text-2xl text-black/70">URL:</label>
              <input
                value={githubImportUrl}
                onChange={(event) => setGithubImportUrl(event.target.value)}
                placeholder="请输入"
                className="h-12 flex-1 rounded-[999px] border border-black/12 bg-white/92 px-4 text-base outline-none focus:ring-1 focus:ring-ring"
              />
            </div>
          </div>
          <DialogFooter className="mt-2 flex-row justify-center gap-3">
            <Button type="button" className="h-12 min-w-[200px] rounded-[999px] bg-black/90 text-lg" onClick={() => void handleGithubImportPlaceholder()}>
              导入
            </Button>
            <Button type="button" variant="outline" className="h-12 min-w-[200px] rounded-[999px] text-lg" onClick={() => setGithubDialogOpen(false)}>
              取消
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Manual Install Dialog */}
      <Dialog open={marketInstallOpen} onOpenChange={setMarketInstallOpen}>
        <DialogContent className="w-[min(92vw,40rem)] rounded-[6px] border border-black/10 bg-[#f7f7f8] p-6">
          <DialogHeader>
            <DialogTitle className="text-2xl font-semibold tracking-tight">Skills Market 手动安装</DialogTitle>
            <DialogDescription className="text-sm text-black/45">
              请提供发布源 URL 与校验信息（checksum + signature），系统会安装到隔离区并进行校验。
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-3">
            <div className="grid grid-cols-[90px_1fr] items-center gap-3">
              <label className="text-sm text-black/70">Skill 名称</label>
              <input
                value={marketInstallName}
                onChange={(event) => setMarketInstallName(event.target.value)}
                className="h-10 rounded-[6px] border border-black/12 bg-white/92 px-3 text-sm outline-none focus:ring-1 focus:ring-ring"
              />
            </div>
            <div className="grid grid-cols-[90px_1fr] items-center gap-3">
              <label className="text-sm text-black/70">URL</label>
              <input
                value={marketInstallUrl}
                onChange={(event) => setMarketInstallUrl(event.target.value)}
                className="h-10 rounded-[6px] border border-black/12 bg-white/92 px-3 text-sm outline-none focus:ring-1 focus:ring-ring"
              />
            </div>
            <div className="grid grid-cols-[90px_1fr] items-center gap-3">
              <label className="text-sm text-black/70">Channel</label>
              <Select
                value={marketInstallChannel}
                onValueChange={(value) => setMarketInstallChannel(value as 'stable' | 'canary')}
              >
                <SelectTrigger className="h-10 rounded-[6px]">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="stable">stable</SelectItem>
                  <SelectItem value="canary">canary</SelectItem>
                </SelectContent>
              </Select>
            </div>
            <div className="grid grid-cols-[90px_1fr] items-center gap-3">
              <label className="text-sm text-black/70">Checksum</label>
              <input
                value={marketInstallChecksum}
                onChange={(event) => setMarketInstallChecksum(event.target.value)}
                placeholder="SHA-256"
                className="h-10 rounded-[6px] border border-black/12 bg-white/92 px-3 text-sm outline-none focus:ring-1 focus:ring-ring"
              />
            </div>
            <div className="grid grid-cols-[90px_1fr] items-center gap-3">
              <label className="text-sm text-black/70">Signature</label>
              <input
                value={marketInstallSignature}
                onChange={(event) => setMarketInstallSignature(event.target.value)}
                placeholder="签名字符串"
                className="h-10 rounded-[6px] border border-black/12 bg-white/92 px-3 text-sm outline-none focus:ring-1 focus:ring-ring"
              />
            </div>
          </div>
          <DialogFooter className="mt-2 flex-row justify-end gap-2">
            <Button type="button" variant="outline" onClick={() => setMarketInstallOpen(false)}>
              取消
            </Button>
            <Button type="button" onClick={() => void handleManualInstallSubmit()}>
              安装
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  )
}

// ─── Status Pill ──────────────────────────────────────────────────────────────

function StatusPill({ status }: { status: string }) {
  const meta: Record<string, { bg: string; text: string; label: string }> = {
    active: { bg: 'status-active-bg', text: 'status-active', label: 'Active' },
    review_passed: { bg: 'status-success-bg', text: 'status-success', label: '已通过' },
    draft: { bg: 'status-pending-bg', text: 'status-pending', label: 'Draft' },
    quarantine: { bg: 'status-warning-bg', text: 'status-warning', label: '隔离' },
    disabled: { bg: 'status-neutral-bg', text: 'status-neutral', label: '已禁用' },
  }
  const m = meta[status] ?? meta.disabled
  return (
    <span
      className="inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[10px] font-medium tracking-wide"
      style={{ background: `var(--${m.bg})`, color: `var(--${m.text})` }}
    >
      <span className="h-1.5 w-1.5 rounded-full shrink-0" style={{ background: `var(--${m.text})` }} />
      {m.label}
    </span>
  )
}

// ─── Agent Proposal Banner ────────────────────────────────────────────────────

/**
 * Displays a highlighted review queue when the Agent has drafted new skills
 * that need human approval before activation.
 *
 * Shown only when at least one skill is in `draft` or `quarantine` status
 * and originates from the `workspace` source (agent-created proposals).
 */
function AgentProposalBanner({
  skills,
  onReview,
  onApprove,
  onRollback,
}: {
  skills: SkillInfo[]
  onReview: (skill: SkillInfo) => void
  onApprove: (skill: SkillInfo) => void
  onRollback: (skill: SkillInfo) => void
}) {
  const proposals = skills.filter(
    (s) =>
      (s.review_status === 'draft' || s.review_status === 'quarantine') &&
      (s.source === 'workspace' || s.source === 'user')
  )

  if (proposals.length === 0) return null

  return (
    <SettingsSurface className="border-amber-200 bg-amber-50">
      <div className="flex items-start gap-3 p-4">
        <BotMessageSquare className="mt-0.5 h-5 w-5 shrink-0 text-amber-600" />
        <div className="flex-1 min-w-0">
          <p className="text-sm font-semibold text-amber-900">
            Agent 提案待审批（{proposals.length} 个）
          </p>
          <p className="mt-0.5 text-xs text-amber-700">
            以下技能由 AI Agent 自动创建，需要你 Review 并批准后才可激活使用。
          </p>
          <ul className="mt-3 space-y-2">
            {proposals.map((skill) => (
              <li
                key={skill.path}
                className="flex items-center justify-between gap-2 rounded-lg border border-amber-200 bg-white/70 px-3 py-2"
              >
                <div className="min-w-0 flex-1">
                  <span className="truncate text-sm font-medium text-amber-900">{skill.name}</span>
                  {skill.description && (
                    <p className="mt-0.5 truncate text-xs text-amber-700/80">{skill.description}</p>
                  )}
                  <div className="mt-1 flex gap-1.5">
                    <span className="rounded-full bg-amber-100 px-2 py-0.5 text-[10px] font-medium text-amber-800 uppercase tracking-wide">
                      {skill.review_status}
                    </span>
                    <span className="rounded-full bg-amber-100 px-2 py-0.5 text-[10px] text-amber-700">
                      {skill.source}
                    </span>
                  </div>
                </div>
                <div className="flex shrink-0 gap-1.5">
                  {skill.review_status === 'draft' && (
                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      className="h-7 rounded-full border-amber-300 px-2.5 text-xs text-amber-800 hover:bg-amber-100"
                      onClick={() => onReview(skill)}
                    >
                      Review
                    </Button>
                  )}
                  {(skill.review_status === 'draft' || skill.review_status === 'review_passed') && (
                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      className="h-7 rounded-full border-green-300 px-2.5 text-xs text-green-800 hover:bg-green-50"
                      onClick={() => onApprove(skill)}
                    >
                      <CheckCircle2 className="mr-1 h-3 w-3" />
                      批准
                    </Button>
                  )}
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    className="h-7 rounded-full px-2 text-xs text-red-600 hover:bg-red-50"
                    onClick={() => onRollback(skill)}
                  >
                    <XCircle className="h-3 w-3" />
                  </Button>
                </div>
              </li>
            ))}
          </ul>
        </div>
      </div>
    </SettingsSurface>
  )
}

// ─── Utility functions ────────────────────────────────────────────────────────

function classifySkill(skill: SkillInfo): SkillCategoryId {
  const text = `${skill.name} ${skill.path} ${skill.description}`.toLowerCase()
  if (containsAny(text, ['browser', 'cdp', 'web'])) return 'web'
  if (containsAny(text, ['doc', 'pdf', 'ppt', 'xls', 'xlsx', 'docx', 'content'])) return 'docs'
  if (containsAny(text, ['search', 'find', 'research'])) return 'search'
  if (containsAny(text, ['email', 'meeting', 'upload', 'cloud', 'kdocs', 'imap', 'smtp'])) return 'collab'
  if (containsAny(text, ['image', 'video', 'media', 'canvas', 'audio'])) return 'media'
  return 'coding'
}

function visualMetaForSkill(skill: SkillInfo) {
  const category = classifySkill(skill)
  switch (category) {
    case 'web':
      return { icon: Globe, iconBgClass: 'bg-sky-100', iconClass: 'text-sky-600' }
    case 'docs':
      return { icon: FileText, iconBgClass: 'bg-emerald-100', iconClass: 'text-emerald-600' }
    case 'search':
      return { icon: Search, iconBgClass: 'bg-indigo-100', iconClass: 'text-indigo-600' }
    default:
      return { icon: Code2, iconBgClass: 'bg-cyan-100', iconClass: 'text-cyan-600' }
  }
}

function categoryLabel(category: SkillCategoryId): string {
  switch (category) {
    case 'docs':
      return '文档内容'
    case 'search':
      return '搜索研究'
    case 'web':
      return '网页自动化'
    case 'collab':
      return '协作通讯'
    case 'media':
      return '多媒体AI'
    case 'recent':
      return '近期使用'
    case 'all':
    case 'coding':
    default:
      return '代码开发'
  }
}

function fallbackDescription(skill: SkillInfo): string {
  if (skill.source === 'builtin') return '通过预置最佳实践快速完成任务。'
  return '用户添加技能，可按需扩展工作流能力。'
}

function sourceLabel(source: SkillInfo['source']): string {
  switch (source) {
    case 'builtin':
      return '内置技能'
    case 'user':
      return '用户技能'
    case 'workspace':
      return '工作区技能'
    case 'remote-quarantine':
      return '隔离技能'
    default:
      return source
  }
}

function loadMarketCache(): MarketCachePayload | null {
  if (typeof window === 'undefined') return null
  try {
    const raw = window.localStorage.getItem(MARKET_CACHE_KEY)
    if (!raw) return null
    const parsed = JSON.parse(raw) as MarketCachePayload
    if (
      !parsed ||
      typeof parsed.updatedAt !== 'number' ||
      !Array.isArray(parsed.data)
    ) {
      return null
    }
    return parsed
  } catch {
    return null
  }
}

function saveMarketCache(payload: MarketCachePayload): void {
  if (typeof window === 'undefined') return
  try {
    window.localStorage.setItem(MARKET_CACHE_KEY, JSON.stringify(payload))
  } catch {
    // Ignore localStorage quota errors.
  }
}

function formatCacheTime(timestamp: number): string {
  const date = new Date(timestamp)
  return date.toLocaleString('zh-CN', { hour12: false })
}

function deriveSkillNameFromGithub(url: string): string | null {
  const cleaned = url.trim().replace(/\/+$/, '')
  const segments = cleaned.split('/')
  const repo = segments[segments.length - 1]
  if (!repo) return null
  return repo.replace(/[^a-zA-Z0-9-_]/g, '-').toLowerCase()
}

function containsAny(text: string, keywords: string[]): boolean {
  return keywords.some((keyword) => text.includes(keyword))
}

function ErrorBanner({ text }: { text: string }) {
  return <div className="rounded-xl border border-red-200 bg-red-50 px-3 py-2 text-[12px] text-red-600">{text}</div>
}

function InfoBanner({ text, tone }: { text: string; tone: 'success' | 'neutral' }) {
  return (
    <div
      className={cn(
        'rounded-xl px-3 py-2 text-[12px]',
        tone === 'success'
          ? 'border border-emerald-200 bg-emerald-50 text-emerald-700'
          : 'border border-black/10 bg-black/[0.02] text-muted-foreground'
      )}
    >
      {text}
    </div>
  )
}

function StatCard({ label, value, hint }: { label: string; value: string; hint: string }) {
  return (
    <div className="rounded-xl border border-border/50 bg-surface-raised p-3 shadow-token-xs">
      <div className="text-xs text-muted-foreground">{label}</div>
      <div className="text-xl font-semibold leading-7">{value}</div>
      <div className="text-[11px] text-muted-foreground">{hint}</div>
    </div>
  )
}
