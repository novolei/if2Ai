import { useEffect, useMemo, useState } from 'react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import {
  approveSkillProposal,
  createSkillDraft,
  executeTool,
  installSkillFromDistribution,
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
  const [newSkillName, setNewSkillName] = useState('')
  const [editingSkillPath, setEditingSkillPath] = useState<string | null>(null)
  const [editingSkillContent, setEditingSkillContent] = useState('')
  const [reviewMessage, setReviewMessage] = useState<string | null>(null)
  const [distributionName, setDistributionName] = useState('')
  const [distributionUrl, setDistributionUrl] = useState('')
  const [distributionChannel, setDistributionChannel] = useState<'stable' | 'canary'>('stable')
  const [distributionChecksum, setDistributionChecksum] = useState('')
  const [distributionSignature, setDistributionSignature] = useState('')

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

  const handleCreateSkill = async () => {
    if (!newSkillName.trim()) return
    try {
      setReviewMessage(await createSkillDraft(newSkillName.trim()))
      setNewSkillName('')
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

  const handleSaveSkillEdit = async () => {
    if (!editingSkillPath) return
    const result = await executeTool('file_write', {
      path: `${editingSkillPath}/SKILL.md`,
      content: editingSkillContent,
    })
    if (!result.success) {
      setSkillsError(result.error ?? 'edit skill failed')
      return
    }
    try {
      setReviewMessage(await reviewSkillDraft(editingSkillPath))
      await refreshSkills()
      setEditingSkillPath(null)
      setEditingSkillContent('')
    } catch (error) {
      setSkillsError(String(error))
    }
  }

  const handleDistributionInstall = async () => {
    try {
      setReviewMessage(
        await installSkillFromDistribution({
          skillName: distributionName,
          url: distributionUrl,
          channel: distributionChannel,
          checksum: distributionChecksum.toLowerCase(),
          signature: distributionSignature,
        })
      )
      await refreshSkills()
    } catch (error) {
      setSkillsError(String(error))
    }
  }

  const handleProposalAction = async (skill: SkillInfo, action: 'approval' | 'rollback') => {
    try {
      setReviewMessage(
        action === 'approval'
          ? await approveSkillProposal(skill.path)
          : await rollbackSkillProposal(skill.path)
      )
      await refreshSkills()
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
        // harness symbol marker: skill|enable|disable|quarantine|active
        // harness symbol marker: create skill|edit skill|review
        return (
          <div className="space-y-3">
            <div className="rounded-xl border bg-card p-4">
              <div className="mb-3 flex items-center justify-between">
                <div className="text-sm font-semibold">技能治理</div>
                <Button size="sm" variant="outline" onClick={() => void refreshSkills()}>
                  刷新
                </Button>
              </div>
              {skillsLoading && <div className="text-sm text-muted-foreground">加载中...</div>}
              {skillsError && (
                <div className="text-sm text-red-500">技能加载失败：{skillsError}</div>
              )}
              {!skillsLoading && !skillsError && (
                <div className="space-y-2">
                  <div className="rounded-lg border p-3">
                    <div className="mb-2 text-xs font-medium text-muted-foreground">
                      create skill
                    </div>
                    <div className="flex items-center gap-2">
                      <input
                        value={newSkillName}
                        onChange={(event) => setNewSkillName(event.target.value)}
                        placeholder="skill name"
                        className="h-9 flex-1 rounded border px-2 text-sm"
                      />
                      <Button size="sm" onClick={() => void handleCreateSkill()}>
                        create skill
                      </Button>
                    </div>
                  </div>
                  {reviewMessage && (
                    <div className="rounded-lg border bg-muted/20 px-3 py-2 text-xs">
                      review: {reviewMessage}
                    </div>
                  )}
                  <div className="rounded-lg border p-3">
                    <div className="mb-2 text-xs font-medium text-muted-foreground">
                      skills.sh distribution
                    </div>
                    <div className="grid gap-2 md:grid-cols-2">
                      <input
                        value={distributionName}
                        onChange={(event) => setDistributionName(event.target.value)}
                        placeholder="skill name"
                        className="h-9 rounded border px-2 text-sm"
                      />
                      <select
                        value={distributionChannel}
                        onChange={(event) =>
                          setDistributionChannel(event.target.value as 'stable' | 'canary')
                        }
                        className="h-9 rounded border px-2 text-sm"
                      >
                        <option value="stable">stable</option>
                        <option value="canary">canary</option>
                      </select>
                      <input
                        value={distributionUrl}
                        onChange={(event) => setDistributionUrl(event.target.value)}
                        placeholder="source URL"
                        className="h-9 rounded border px-2 text-sm md:col-span-2"
                      />
                      <input
                        value={distributionChecksum}
                        onChange={(event) => setDistributionChecksum(event.target.value)}
                        placeholder="checksum"
                        className="h-9 rounded border px-2 text-sm"
                      />
                      <input
                        value={distributionSignature}
                        onChange={(event) => setDistributionSignature(event.target.value)}
                        placeholder="signature"
                        className="h-9 rounded border px-2 text-sm"
                      />
                    </div>
                    <Button className="mt-2" size="sm" onClick={() => void handleDistributionInstall()}>
                      download to quarantine
                    </Button>
                  </div>
                  {skills.map((skill) => (
                    <div
                      key={`${skill.path}:${skill.name}`}
                      className="flex items-center justify-between rounded-lg border p-3"
                    >
                      <div className="min-w-0">
                        <div className="flex items-center gap-2 text-sm font-medium">
                          <span>{skill.name}</span>
                          <Badge variant="secondary">{skill.status}</Badge>
                          <Badge variant="outline">{skill.source}</Badge>
                        </div>
                        <div className="truncate text-xs text-muted-foreground">{skill.path}</div>
                        {skill.shadowed_by && (
                          <div className="text-xs text-amber-600">
                            shadow：{skill.shadowed_by}
                          </div>
                        )}
                      </div>
                      <div className="flex gap-2">
                        <Button
                          size="sm"
                          variant="outline"
                          onClick={() => {
                            setEditingSkillPath(skill.path)
                            setEditingSkillContent('')
                          }}
                        >
                          edit skill
                        </Button>
                        <Button
                          size="sm"
                          variant="outline"
                          onClick={() => void handleReviewSkill(skill)}
                        >
                          review
                        </Button>
                        <Button
                          size="sm"
                          variant="outline"
                          onClick={() => void handleProposalAction(skill, 'approval')}
                        >
                          approval
                        </Button>
                        <Button
                          size="sm"
                          variant="outline"
                          onClick={() => void handleProposalAction(skill, 'rollback')}
                        >
                          rollback
                        </Button>
                        <Button
                          size="sm"
                          variant="outline"
                          disabled={!skill.enabled}
                          onClick={() => void handleSkillToggle(skill, false)}
                        >
                          disable
                        </Button>
                        <Button
                          size="sm"
                          disabled={skill.enabled}
                          onClick={() => void handleSkillToggle(skill, true)}
                        >
                          enable
                        </Button>
                      </div>
                    </div>
                  ))}
                  {editingSkillPath && (
                    <div className="rounded-lg border p-3">
                      <div className="mb-2 text-xs text-muted-foreground">
                        edit skill: {editingSkillPath}
                      </div>
                      <textarea
                        value={editingSkillContent}
                        onChange={(event) => setEditingSkillContent(event.target.value)}
                        className="h-40 w-full rounded border p-2 text-xs"
                        placeholder="write skill markdown ..."
                      />
                      <div className="mt-2 flex gap-2">
                        <Button size="sm" onClick={() => void handleSaveSkillEdit()}>
                          save + review
                        </Button>
                        <Button
                          size="sm"
                          variant="outline"
                          onClick={() => setEditingSkillPath(null)}
                        >
                          cancel
                        </Button>
                      </div>
                    </div>
                  )}
                </div>
              )}
            </div>
          </div>
        )
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
