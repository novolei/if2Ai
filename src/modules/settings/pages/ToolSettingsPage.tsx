/**
 * ToolSettingsPage — user-facing entry point for AI tool runtime parameters.
 *
 * Currently surfaces the **Browser** tool (Phase 7C):
 *   - profile_mode dropdown (per_session_persistent / shared / ephemeral)
 *   - max_profile_disk_mb / max_total_disk_mb soft caps
 *   - live profile inventory with disk usage + clear / refresh actions
 *   - banner when env-var override or "next launch needed" is in effect
 *
 * Future tools (Files / Memory / Cron) can land additional sections in
 * this same page without growing the sidebar.
 */
import { useCallback, useEffect, useState } from 'react'
import { toast } from 'sonner'
import { AlertCircle, FolderOpen, RefreshCw, Trash2 } from 'lucide-react'
import { SettingsSurface } from '../components/SettingsSurface'
import { SettingsRow } from '../components/SettingsRow'
import { CompactInput } from '../components/CompactInput'
import {
  clearBrowserProfile,
  getBrowserSessions,
  getBrowserSettings,
  getChromeStatus,
  listBrowserProfiles,
  setBrowserSettings,
  type BrowserProfileEntry,
  type BrowserProfileMode,
  type BrowserSettings,
  type ChromeStatusPayload,
} from '@/lib/tauri'
import { cn } from '@/lib/utils'

// ── Helpers ──────────────────────────────────────────────────────────────────

const PROFILE_MODE_LABEL: Record<BrowserProfileMode, string> = {
  per_session_persistent: '每会话独立 (推荐)',
  shared: '所有会话共享',
  ephemeral: '一次性 (匿名)',
}

const PROFILE_MODE_HINT: Record<BrowserProfileMode, string> = {
  per_session_persistent:
    '每个 chat session 各自一份持久 profile，cookies 跨重启保留，会话之间互不可见。',
  shared:
    '所有会话共享同一个 profile（如同一个 Gmail 登录给所有 AI 看）。隐私警示：会话之间能互相看到 cookies。',
  ephemeral:
    '每次启动建临时 profile，进程退出全部清空 (cookies / cache 不保留)。适合测试 / 红队 / 一次性场景。',
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  const kb = bytes / 1024
  if (kb < 1024) return `${kb.toFixed(1)} KB`
  const mb = kb / 1024
  if (mb < 1024) return `${mb.toFixed(1)} MB`
  return `${(mb / 1024).toFixed(2)} GB`
}

function formatDate(iso: string | null): string {
  if (!iso) return '—'
  try {
    const d = new Date(iso)
    return `${d.toLocaleDateString('zh-CN')} ${d.toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' })}`
  } catch {
    return iso
  }
}

// ── Component ────────────────────────────────────────────────────────────────

export function ToolSettingsPage() {
  const [settings, setSettings] = useState<BrowserSettings | null>(null)
  const [profiles, setProfiles] = useState<BrowserProfileEntry[]>([])
  const [chromeStatus, setChromeStatus] = useState<ChromeStatusPayload | null>(null)
  const [activeSessionIds, setActiveSessionIds] = useState<Set<string>>(new Set())
  const [loading, setLoading] = useState(true)
  const [savingMode, setSavingMode] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const refresh = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      const [s, p, c, sessions] = await Promise.all([
        getBrowserSettings(),
        listBrowserProfiles(),
        getChromeStatus().catch(() => null),
        getBrowserSessions().catch(() => []),
      ])
      setSettings(s)
      setProfiles(p)
      setChromeStatus(c)
      setActiveSessionIds(new Set(sessions.map((row) => row.session_id)))
    } catch (err) {
      setError(String(err))
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    void refresh()
  }, [refresh])

  const handleModeChange = async (next: BrowserProfileMode) => {
    if (!settings) return
    const updated: BrowserSettings = { ...settings, profile_mode: next }
    setSavingMode(true)
    try {
      await setBrowserSettings(updated)
      setSettings(updated)
      toast.success('已保存', {
        description:
          updated.profile_mode === settings.active_mode
            ? '设置即时生效。'
            : '下次启动 AI 浏览器时生效；当前已运行的会话保持原模式。',
      })
    } catch (err) {
      toast.error('保存失败', { description: String(err) })
    } finally {
      setSavingMode(false)
    }
  }

  const handleNumChange = async (
    field: 'max_profile_disk_mb' | 'max_total_disk_mb',
    raw: string,
  ) => {
    if (!settings) return
    const parsed = Number(raw)
    if (!Number.isFinite(parsed) || parsed <= 0) {
      toast.error('请输入大于 0 的数字')
      return
    }
    const updated: BrowserSettings = { ...settings, [field]: Math.round(parsed) }
    try {
      await setBrowserSettings(updated)
      setSettings(updated)
    } catch (err) {
      toast.error('保存失败', { description: String(err) })
    }
  }

  const handleClearProfile = async (entry: BrowserProfileEntry) => {
    if (entry.session_id === '_shared') {
      toast.error('不支持通过 UI 清除 shared profile', {
        description: '请关闭所有 session 后手动删除目录。',
      })
      return
    }
    if (activeSessionIds.has(entry.session_id)) {
      toast.error('清除前请先关闭该会话', {
        description: `Session ${entry.session_id} 仍在运行；先到主窗口关闭浏览器卡片。`,
      })
      return
    }
    if (
      !window.confirm(
        `永久删除 session "${entry.session_id}" 的 cookies / 登录态 / 缓存？此操作不可撤销。`,
      )
    ) {
      return
    }
    try {
      await clearBrowserProfile(entry.session_id)
      toast.success(`已清除 session ${entry.session_id} 的 profile`)
      await refresh()
    } catch (err) {
      toast.error('清除失败', { description: String(err) })
    }
  }

  const totalDisk = profiles.reduce((acc, p) => acc + p.size_bytes, 0)

  return (
    <div className="space-y-5 p-5">
      {/* Section: Header banner */}
      {settings?.env_override ? (
        <div className="flex items-start gap-2.5 rounded-xl border border-amber-300/40 bg-amber-50 px-3.5 py-2.5">
          <AlertCircle className="mt-[1px] h-4 w-4 shrink-0 text-amber-600" />
          <div className="text-[12px] leading-5 text-amber-900">
            <strong>环境变量覆盖中：</strong>
            <code className="rounded bg-amber-100 px-1.5 py-px text-[11px] font-mono">
              IF2AI_BROWSER_PROFILE_MODE={settings.env_override}
            </code>
            。在 env 被清除前，下方 dropdown 的设置不会生效。
          </div>
        </div>
      ) : null}

      {settings && settings.active_mode !== settings.profile_mode ? (
        <div className="flex items-start gap-2.5 rounded-xl border border-blue-300/40 bg-blue-50 px-3.5 py-2.5">
          <AlertCircle className="mt-[1px] h-4 w-4 shrink-0 text-blue-600" />
          <div className="text-[12px] leading-5 text-blue-900">
            当前运行中的 AI 浏览器使用
            <code className="mx-1 rounded bg-blue-100 px-1.5 py-px text-[11px] font-mono">
              {settings.active_mode}
            </code>
            模式，下次重启 If2Ai 后切换为
            <code className="mx-1 rounded bg-blue-100 px-1.5 py-px text-[11px] font-mono">
              {settings.profile_mode}
            </code>
            。
          </div>
        </div>
      ) : null}

      {/* Section: Browser */}
      <SettingsSurface>
        <div className="border-b border-black/[0.06] px-5 py-3.5">
          <div className="flex items-center justify-between">
            <div>
              <div className="text-[14px] font-semibold tracking-tight">浏览器工具</div>
              <div className="mt-0.5 text-[11.5px] text-muted-foreground">
                控制 AI 浏览器的 profile 位置和持久化策略 (Phase 7C / ADR-015)。
              </div>
            </div>
            <button
              type="button"
              onClick={() => void refresh()}
              disabled={loading}
              className={cn(
                'flex h-7 items-center gap-1.5 rounded-lg border border-black/[0.08] bg-white px-2.5 text-[11.5px] font-medium',
                'transition-colors hover:bg-black/[0.03]',
                'disabled:opacity-50 disabled:cursor-not-allowed',
              )}
            >
              <RefreshCw className={cn('h-3 w-3', loading && 'animate-spin')} />
              刷新
            </button>
          </div>
        </div>

        <div className="space-y-1 px-4 py-2">
          <SettingsRow
            inline
            title="Profile 模式"
            description={settings ? PROFILE_MODE_HINT[settings.profile_mode] : '加载中…'}
          >
            <select
              disabled={!settings || savingMode || !!settings.env_override}
              value={settings?.profile_mode ?? 'per_session_persistent'}
              onChange={(e) =>
                void handleModeChange(e.target.value as BrowserProfileMode)
              }
              className={cn(
                'h-8 rounded-xl border border-black/[0.09] bg-black/[0.02] px-3 text-[12px] font-medium',
                'outline-none focus:border-jade/40 focus:ring-[3px] focus:ring-jade/15',
                'disabled:opacity-40 disabled:cursor-not-allowed',
              )}
              style={{ minWidth: 220 }}
            >
              <option value="per_session_persistent">
                {PROFILE_MODE_LABEL.per_session_persistent}
              </option>
              <option value="shared">{PROFILE_MODE_LABEL.shared}</option>
              <option value="ephemeral">{PROFILE_MODE_LABEL.ephemeral}</option>
            </select>
          </SettingsRow>

          <SettingsRow
            inline
            title="单 profile 软上限 (MB)"
            description="用于未来 LRU 清理；当前仅用于 UI 展示。"
          >
            <CompactInput
              type="number"
              min={1}
              defaultValue={settings?.max_profile_disk_mb ?? 500}
              onBlur={(e) => void handleNumChange('max_profile_disk_mb', e.target.value)}
              disabled={!settings}
              style={{ width: 120 }}
            />
          </SettingsRow>

          <SettingsRow
            inline
            title="所有 profile 总上限 (MB)"
            description="用于未来 LRU 清理；当前仅用于 UI 展示。"
          >
            <CompactInput
              type="number"
              min={1}
              defaultValue={settings?.max_total_disk_mb ?? 5000}
              onBlur={(e) => void handleNumChange('max_total_disk_mb', e.target.value)}
              disabled={!settings}
              style={{ width: 120 }}
            />
          </SettingsRow>
        </div>

        {chromeStatus ? (
          <div className="border-t border-black/[0.06] bg-black/[0.014] px-5 py-3 text-[11.5px]">
            {chromeStatus.found ? (
              <span className="text-muted-foreground">
                ✅ 检测到 Chrome：
                <code className="ml-1 rounded bg-black/[0.04] px-1.5 py-px font-mono text-[11px]">
                  {chromeStatus.path}
                </code>
              </span>
            ) : (
              <span className="text-amber-700">
                ⚠️ 未检测到 Chrome / Chromium。请安装 Google Chrome 后重启 If2Ai。
              </span>
            )}
          </div>
        ) : null}
      </SettingsSurface>

      {/* Section: Profile inventory */}
      <SettingsSurface>
        <div className="flex items-center justify-between border-b border-black/[0.06] px-5 py-3.5">
          <div>
            <div className="text-[14px] font-semibold tracking-tight">
              磁盘上的 Profile ({profiles.length})
            </div>
            <div className="mt-0.5 text-[11.5px] text-muted-foreground">
              累计占用 {formatBytes(totalDisk)}。清除即丢失对应 session 的 cookies / 登录态。
            </div>
          </div>
        </div>

        {error ? (
          <div className="px-5 py-4 text-[12px] text-red-600">{error}</div>
        ) : profiles.length === 0 ? (
          <div className="px-5 py-8 text-center text-[12px] text-muted-foreground">
            还没有任何 profile —— AI 第一次使用浏览器时会自动创建。
          </div>
        ) : (
          <div className="divide-y divide-black/[0.05]">
            {profiles.map((entry) => {
              const isShared = entry.session_id === '_shared'
              const isActive = activeSessionIds.has(entry.session_id)
              return (
                <div
                  key={entry.session_id}
                  className="flex items-center justify-between gap-3 px-5 py-2.5 hover:bg-black/[0.012]"
                >
                  <div className="flex min-w-0 items-center gap-2.5">
                    <FolderOpen className="h-4 w-4 shrink-0 text-muted-foreground" />
                    <div className="min-w-0">
                      <div className="flex items-center gap-2">
                        <div className="truncate text-[12.5px] font-medium">
                          {isShared ? '共享 profile' : entry.session_id}
                        </div>
                        {isActive ? (
                          <span className="rounded-full bg-emerald-100 px-1.5 py-px text-[10px] font-medium text-emerald-700">
                            运行中
                          </span>
                        ) : null}
                        {isShared ? (
                          <span className="rounded-full bg-purple-100 px-1.5 py-px text-[10px] font-medium text-purple-700">
                            shared
                          </span>
                        ) : null}
                      </div>
                      <div className="mt-0.5 truncate text-[10.5px] text-muted-foreground">
                        {entry.path}
                      </div>
                    </div>
                  </div>

                  <div className="flex shrink-0 items-center gap-3">
                    <div className="text-right text-[11px] text-muted-foreground">
                      <div className="font-medium tabular-nums text-foreground">
                        {formatBytes(entry.size_bytes)}
                      </div>
                      <div>{formatDate(entry.last_used)}</div>
                    </div>
                    <button
                      type="button"
                      onClick={() => void handleClearProfile(entry)}
                      disabled={isShared || isActive}
                      title={
                        isShared
                          ? '不支持通过 UI 清除共享 profile'
                          : isActive
                          ? '请先关闭该会话'
                          : '清除该 profile'
                      }
                      className={cn(
                        'flex h-7 w-7 items-center justify-center rounded-lg border border-black/[0.08] bg-white',
                        'text-red-500 transition-colors hover:bg-red-50',
                        'disabled:opacity-30 disabled:cursor-not-allowed disabled:hover:bg-white',
                      )}
                    >
                      <Trash2 className="h-3.5 w-3.5" />
                    </button>
                  </div>
                </div>
              )
            })}
          </div>
        )}
      </SettingsSurface>
    </div>
  )
}
