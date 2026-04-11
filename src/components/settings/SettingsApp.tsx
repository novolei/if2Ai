import { useState } from 'react'
import { X, Settings, User, Info, Key, Palette, Sparkles } from 'lucide-react'
import { cn } from '@/lib/utils'
import navStyles from './_settings-nav.module.css'
import sectionStyles from './_settings-section.module.css'
import fieldStyles from './_settings-field.module.css'
import toggleStyles from './_settings-toggle.module.css'
import apiStyles from './_settings-api.module.css'
import aboutStyles from './_settings-about.module.css'

// Tab definitions
const TABS = [
  { id: 'general', label: '通用', icon: Settings },
  { id: 'appearance', label: '外观', icon: Palette },
  { id: 'account', label: '账户', icon: User },
  { id: 'api', label: 'API', icon: Key },
  { id: 'about', label: '关于', icon: Info },
] as const

type TabId = typeof TABS[number]['id']

interface SettingsAppProps {
  onClose: () => void
}

export function SettingsApp({ onClose }: SettingsAppProps) {
  const [activeTab, setActiveTab] = useState<TabId>('general')

  return (
    <div className="flex flex-col h-screen" style={{ backgroundColor: 'var(--color-bg-app)' }}>
      {/* Header */}
      <header
        className="flex items-center justify-between px-6 h-14 shrink-0 border-b"
        style={{ borderColor: 'var(--color-border-soft)' }}
      >
        <h1 className="text-sm font-medium" style={{ color: 'var(--color-text-primary)' }}>设置</h1>
        <button
          onClick={onClose}
          className="p-1.5 rounded-md transition-colors"
          style={{ color: 'var(--color-text-secondary)' }}
          onMouseEnter={(e) => e.currentTarget.style.backgroundColor = 'var(--color-primary-soft)'}
          onMouseLeave={(e) => e.currentTarget.style.backgroundColor = 'transparent'}
        >
          <X className="h-4 w-4" />
        </button>
      </header>

      {/* Body */}
      <div className="flex flex-1 min-h-0">
        {/* Nav */}
        <nav className={navStyles.nav}>
          {TABS.map((tab) => {
            const Icon = tab.icon
            return (
              <button
                key={tab.id}
                className={cn(
                  navStyles.navItem,
                  activeTab === tab.id && navStyles.active
                )}
                onClick={() => setActiveTab(tab.id)}
              >
                <Icon className="h-4 w-4" />
                <span>{tab.label}</span>
              </button>
            )
          })}
        </nav>

        {/* Main Content */}
        <main className="flex-1 overflow-y-auto p-6">
          {activeTab === 'general' && <GeneralSettings />}
          {activeTab === 'appearance' && <AppearanceSettings />}
          {activeTab === 'account' && <AccountSettings />}
          {activeTab === 'api' && <ApiSettings />}
          {activeTab === 'about' && <AboutSettings />}
        </main>
      </div>
    </div>
  )
}

// General Settings
function GeneralSettings() {
  const [language, setLanguage] = useState('zh-CN')
  const [autoScroll, setAutoScroll] = useState(true)

  return (
    <div>
      <section className={sectionStyles.section}>
        <h2 className={sectionStyles.sectionTitle}>通用设置</h2>

        <div className={fieldStyles.field}>
          <label className={fieldStyles.fieldLabel}>语言</label>
          <select
            className={fieldStyles.select}
            value={language}
            onChange={(e) => setLanguage(e.target.value)}
          >
            <option value="zh-CN">简体中文</option>
            <option value="en">English</option>
          </select>
        </div>

        <div className={fieldStyles.fieldRow}>
          <span className={fieldStyles.fieldRowLabel}>自动滚动</span>
          <div
            className={cn(toggleStyles.toggle, autoScroll && toggleStyles.active)}
            onClick={() => setAutoScroll(!autoScroll)}
          />
        </div>
        <p className={sectionStyles.hint}>AI 回复时自动滚动到最新消息</p>
      </section>
    </div>
  )
}

// Appearance Settings
function AppearanceSettings() {
  const [theme, setTheme] = useState('system')
  const [fontMode, setFontMode] = useState<'serif' | 'sans'>(() => {
    return (localStorage.getItem('fontMode') as 'serif' | 'sans') || 'serif'
  })

  const toggleFontMode = () => {
    const newMode = fontMode === 'serif' ? 'sans' : 'serif'
    setFontMode(newMode)
    localStorage.setItem('fontMode', newMode)
  }

  return (
    <div>
      <section className={sectionStyles.section}>
        <h2 className={sectionStyles.sectionTitle}>外观设置</h2>

        <div className={fieldStyles.field}>
          <label className={fieldStyles.fieldLabel}>主题</label>
          <select
            className={fieldStyles.select}
            value={theme}
            onChange={(e) => setTheme(e.target.value)}
          >
            <option value="system">跟随系统</option>
            <option value="light">浅色</option>
            <option value="dark">深色</option>
          </select>
        </div>

        <div className={fieldStyles.fieldRow}>
          <span className={fieldStyles.fieldRowLabel}>聊天字体</span>
          <div
            className={cn(
              toggleStyles.toggle,
              fontMode === 'sans' && toggleStyles.active
            )}
            onClick={toggleFontMode}
          />
        </div>
        <p className={sectionStyles.hint}>
          {fontMode === 'serif' ? '衬线字体（默认）' : '非衬线字体'}
        </p>
      </section>
    </div>
  )
}

// Account Settings
function AccountSettings() {
  const [username, setUsername] = useState('')
  const [email, setEmail] = useState('')

  return (
    <div>
      <section className={sectionStyles.section}>
        <h2 className={sectionStyles.sectionTitle}>账户设置</h2>

        <div className={fieldStyles.field}>
          <label className={fieldStyles.fieldLabel}>用户名</label>
          <input
            type="text"
            className={fieldStyles.input}
            value={username}
            onChange={(e) => setUsername(e.target.value)}
            placeholder="输入用户名"
          />
        </div>

        <div className={fieldStyles.field}>
          <label className={fieldStyles.fieldLabel}>邮箱</label>
          <input
            type="email"
            className={fieldStyles.input}
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            placeholder="输入邮箱"
          />
        </div>
      </section>
    </div>
  )
}

// API Settings
function ApiSettings() {
  const [apiKey, setApiKey] = useState('')
  const [baseUrl, setBaseUrl] = useState('')
  const [isConnected, setIsConnected] = useState(false)

  return (
    <div>
      <section className={sectionStyles.section}>
        <h2 className={sectionStyles.sectionTitle}>API 配置</h2>

        <div className={apiStyles.apiSection}>
          <div className={apiStyles.apiHeader}>
            <span className={apiStyles.apiTitle}>Anthropic API</span>
            <span className={cn(
              apiStyles.apiStatus,
              isConnected ? apiStyles.apiStatusConnected : apiStyles.apiStatusDisconnected
            )}>
              {isConnected ? '已连接' : '未连接'}
            </span>
          </div>

          <div className={fieldStyles.field}>
            <label className={fieldStyles.fieldLabel}>API Key</label>
            <input
              type="password"
              className={fieldStyles.input}
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              placeholder="sk-ant-..."
            />
          </div>

          <div className={fieldStyles.field}>
            <label className={fieldStyles.fieldLabel}>Base URL</label>
            <input
              type="text"
              className={fieldStyles.input}
              value={baseUrl}
              onChange={(e) => setBaseUrl(e.target.value)}
              placeholder="https://api.anthropic.com"
            />
          </div>

          <button className={apiStyles.saveBtn}>保存</button>
        </div>
      </section>
    </div>
  )
}

// About Settings
function AboutSettings() {
  return (
    <div>
      <section className={sectionStyles.section}>
        <div className={aboutStyles.about}>
          <div
            className="w-16 h-16 rounded-2xl flex items-center justify-center mb-4"
            style={{ background: 'linear-gradient(135deg, var(--color-primary) 0%, var(--color-accent-mint) 100%)' }}
          >
            <Sparkles className="h-8 w-8" style={{ color: 'var(--color-text-inverse)' }} />
          </div>
          <h2 className={aboutStyles.aboutName}>If2Ai</h2>
          <p className={aboutStyles.aboutVersion}>版本 0.1.0</p>
        </div>
      </section>

      <section className={sectionStyles.section}>
        <h2 className={sectionStyles.sectionTitle}>关于</h2>
        <p className={aboutStyles.aboutDescription}>
          If2Ai 是一个基于 Tauri 2 + React 的桌面应用程序，
          提供 AI 对话功能，支持多种 AI 提供商。
        </p>
      </section>
    </div>
  )
}
