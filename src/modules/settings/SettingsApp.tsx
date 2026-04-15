import { useEffect, useMemo, useState } from 'react'
import { toast } from 'sonner'
import {
  approveSkillProposal,
  focusMainWindowAndPrefillPrompt,
  listSkills,
  reviewSkillDraft,
  rollbackSkillProposal,
  setSkillEnabled,
  type SkillInfo,
} from '@/lib/tauri'
import type { SettingsActions, SettingsSectionId, SettingsState, ThemeMode, FontMode } from './types'
import { SettingsShell } from './components/SettingsShell'
import { GeneralSettingsPage } from './pages/GeneralSettingsPage'
import { UsageSettingsPage } from './pages/UsageSettingsPage'
import { ConnectionsSettingsPage } from './pages/ConnectionsSettingsPage'
import { RemoteSettingsPage } from './pages/RemoteSettingsPage'
import { AboutSettingsPage } from './pages/AboutSettingsPage'
import { SkillsSettingsPage } from './pages/SkillsSettingsPage'
import { WebSearchSettingsPage } from './pages/WebSearchSettingsPage'
import { MemorySettingsPage } from './pages/MemorySettingsPage'

interface SettingsAppProps {
  onClose: () => void
}

const DEFAULT_FONT_MODE: FontMode = 'sans'

export function SettingsApp({ onClose }: SettingsAppProps) {
  const [activeSection, setActiveSection] = useState<SettingsSectionId>('general')
  const [theme, setTheme] = useState<ThemeMode>('system')
  const [fontMode, setFontMode] = useState<FontMode>(DEFAULT_FONT_MODE)
  const [language, setLanguage] = useState('zh-CN')
  const [startupMode, setStartupMode] = useState('last')
  const [density, setDensity] = useState('comfortable')
  const [autoScroll, setAutoScroll] = useState(true)
  const [notifications, setNotifications] = useState(true)
  const [username, setUsername] = useState('')
  const [email, setEmail] = useState('')
  const [apiKey, setApiKey] = useState('')
  const [baseUrl, setBaseUrl] = useState('')
  const [skills, setSkills] = useState<SkillInfo[]>([])
  const [skillsLoading, setSkillsLoading] = useState(false)
  const [skillsError, setSkillsError] = useState<string | null>(null)

  const refreshSkills = async () => {
    setSkillsLoading(true)
    setSkillsError(null)
    try {
      setSkills(await listSkills())
    } catch (error) {
      setSkillsError(String(error))
    } finally {
      setSkillsLoading(false)
    }
  }

  const handleSkillToggle = async (skill: SkillInfo, enabled: boolean) => {
    const action = enabled ? '启用' : '禁用'
    if (!window.confirm(`确认${action}技能「${skill.name}」吗？`)) return
    try {
      await setSkillEnabled(skill.path, enabled)
      toast.success(`已${enabled ? '启用' : '禁用'}「${skill.name}」`)
      await refreshSkills()
    } catch (error) {
      toast.error(`操作失败：${skill.name}`, { description: String(error) })
    }
  }

  const handleReviewSkill = async (skill: SkillInfo) => {
    try {
      toast.loading(`正在 Review「${skill.name}」…`, { id: `review-${skill.path}` })
      const msg = await reviewSkillDraft(skill.path)
      toast.success(`「${skill.name}」Review 完成`, { description: msg })
      await refreshSkills()
    } catch (error) {
      toast.error(`Review 失败：${skill.name}`, { description: String(error) })
    }
  }

  const handleProposalAction = async (skill: SkillInfo, action: 'approval' | 'rollback') => {
    try {
      if (action === 'approval') {
        // draft 状态先自动 review 一次，再执行 approve，实现"一键批准"
        if (skill.review_status === 'draft') {
          await reviewSkillDraft(skill.path)
        }
        const msg = await approveSkillProposal(skill.path)
        toast.success(`「${skill.name}」已批准`, { description: msg })
      } else {
        const msg = await rollbackSkillProposal(skill.path)
        toast.info(`「${skill.name}」已回滚`, { description: msg })
      }
      await refreshSkills()
    } catch (error) {
      toast.error(`操作失败：${skill.name}`, { description: String(error) })
    }
  }

  const handleStartConversationCreate = async () => {
    const prompt =
      '跟我一起用/skill-creator 创建一个技能，并且加入我的技能文档/列表 ~/.qclaw/skills 里。现在，你先问我技能应该做什么吧。'
    try {
      await focusMainWindowAndPrefillPrompt(prompt)
      toast.success('已跳转到主窗口', { description: '开始创建技能对话' })
    } catch (error) {
      toast.error('跳转失败', { description: String(error) })
    }
  }

  useEffect(() => {
    const savedFontMode = localStorage.getItem('fontMode') as FontMode | null
    if (savedFontMode) {
      setFontMode(savedFontMode)
    }
  }, [])

  useEffect(() => {
    localStorage.setItem('fontMode', fontMode)
    document.body.classList.toggle('font-serif-mode', fontMode === 'serif')
  }, [fontMode])

  useEffect(() => {
    if (activeSection === 'skills') {
      void refreshSkills()
    }
  }, [activeSection])

  const state: SettingsState = {
    theme,
    fontMode,
    language,
    startupMode,
    density,
    autoScroll,
    notifications,
    username,
    email,
    apiKey,
    baseUrl,
  }

  const actions: SettingsActions = useMemo(
    () => ({
      setTheme,
      setFontMode,
      setLanguage,
      setStartupMode,
      setDensity,
      setAutoScroll,
      setNotifications,
      setUsername,
      setEmail,
      setApiKey,
      setBaseUrl,
    }),
    [],
  )

  const content = (() => {
    switch (activeSection) {
      case 'usage':
        return <UsageSettingsPage state={state} actions={actions} />
      case 'skills':
        return (
          <SkillsSettingsPage
            skills={skills}
            loading={skillsLoading}
            error={skillsError}
            onRefresh={() => void refreshSkills()}
            onToggleSkill={(skill, enabled) => void handleSkillToggle(skill, enabled)}
            onStartConversationCreate={() => void handleStartConversationCreate()}
            onReviewSkill={(skill) => void handleReviewSkill(skill)}
            onApproveSkill={(skill) => void handleProposalAction(skill, 'approval')}
            onRollbackSkill={(skill) => void handleProposalAction(skill, 'rollback')}
          />
        )
      case 'web-search':
        return <WebSearchSettingsPage />
      case 'memory':
        return <MemorySettingsPage />
      case 'connections':
        return <ConnectionsSettingsPage state={state} actions={actions} />
      case 'remote':
        return <RemoteSettingsPage state={state} actions={actions} />
      case 'about':
        return <AboutSettingsPage state={state} actions={actions} />
      case 'general':
      default:
        return <GeneralSettingsPage state={state} actions={actions} />
    }
  })()

  return (
    <SettingsShell
      activeSection={activeSection}
      onSectionChange={setActiveSection}
      onClose={onClose}
    >
      {content}
    </SettingsShell>
  )
}

export type { SettingsAppProps }
