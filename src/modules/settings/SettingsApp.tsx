import { useEffect, useMemo, useState } from 'react'
import type { SettingsActions, SettingsSectionId, SettingsState, ThemeMode, FontMode } from './types'
import { SettingsShell } from './components/SettingsShell'
import { GeneralSettingsPage } from './pages/GeneralSettingsPage'
import { UsageSettingsPage } from './pages/UsageSettingsPage'
import { SkillsSettingsPage } from './pages/SkillsSettingsPage'
import { ConnectionsSettingsPage } from './pages/ConnectionsSettingsPage'
import { RemoteSettingsPage } from './pages/RemoteSettingsPage'
import { AboutSettingsPage } from './pages/AboutSettingsPage'

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
        return <SkillsSettingsPage state={state} actions={actions} />
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
