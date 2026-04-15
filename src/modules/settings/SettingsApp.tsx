import { useEffect, useMemo, useState } from 'react'
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
  const [reviewMessage, setReviewMessage] = useState<string | null>(null)

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
      await refreshSkills()
    } catch (error) {
      setSkillsError(String(error))
    }
  }

  const handleReviewSkill = async (skill: SkillInfo) => {
    try {
      setReviewMessage(await reviewSkillDraft(skill.path))
      await refreshSkills()
    } catch (error) {
      setSkillsError(String(error))
    }
  }

  const handleProposalAction = async (skill: SkillInfo, action: 'approval' | 'rollback') => {
    try {
      if (action === 'approval') {
        // draft 状态先自动 review 一次，再执行 approve，实现"一键批准"
        if (skill.review_status === 'draft') {
          await reviewSkillDraft(skill.path)
        }
        setReviewMessage(await approveSkillProposal(skill.path))
      } else {
        setReviewMessage(await rollbackSkillProposal(skill.path))
      }
      await refreshSkills()
    } catch (error) {
      setSkillsError(String(error))
    }
  }

  const handleStartConversationCreate = async () => {
    const prompt =
      '跟我一起用/skill-creator 创建一个技能，并且加入我的技能文档/列表 ~/.qclaw/skills 里。现在，你先问我技能应该做什么吧。'
    try {
      await focusMainWindowAndPrefillPrompt(prompt)
    } catch (error) {
      setSkillsError(String(error))
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
            reviewMessage={reviewMessage}
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
