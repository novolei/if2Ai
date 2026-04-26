/**
 * BrowserCard — floating status card for an AI-controlled browser session.
 *
 * Listens to the Tauri `"browser-status"` event, updates the module-level
 * `browserBySession` store, and renders a compact overlay card showing:
 *
 * - Live viewport thumbnail (base-64 JPEG, updated after each browser action)
 * - Current page hostname
 * - "Running" pulse indicator
 * - Emergency-stop button (calls `close_browser_session`)
 *
 * The card is only rendered when `entry.running === true`.
 * The Tauri event listener is cleaned up on component unmount.
 */

import { useEffect, useState } from 'react'
import { Expand, Globe, Hand, Pause, Square } from 'lucide-react'
import { toast } from 'sonner'
import { cn } from '@/lib/utils'
import { SmartBrowserCockpit } from '@/modules/smart-browser/SmartBrowserCockpit'
import {
  closeBrowserSession,
  approveSmartBrowserCloudEscalation,
  denySmartBrowserCloudEscalation,
  listenToBrowserStatus,
  openBrowserViewerWindow,
  releaseBrowserTakeover,
  requestBrowserTakeover,
  requestSmartBrowserCloudEscalation,
} from '@/lib/tauri'
import {
  useBrowserStore,
  setBrowserStatus,
  clearBrowserSession,
} from '@/stores/browser-slice'

// ── Props ─────────────────────────────────────────────────────────────────────

interface BrowserCardProps {
  /** The `session_id` of the chat session whose browser to display. */
  sessionId: string
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/** Safely extract the hostname from a URL string. */
function hostnameOf(url: string | null): string {
  if (!url) return '…'
  try {
    return new URL(url).hostname
  } catch {
    return url
  }
}

// ── Component ─────────────────────────────────────────────────────────────────

/**
 * Floating card that tracks live browser state for `sessionId`.
 * Returns `null` (renders nothing) when no browser is running.
 */
export function BrowserCard({ sessionId }: BrowserCardProps) {
  const { browserBySession } = useBrowserStore()
  const entry = browserBySession[sessionId]

  // Phase 7C, slice 7C.3 — local "user has taken over" state.
  // Mirrors the backend `BrowserRegistry::is_taken_over` flag; we keep
  // a copy in the React tree so the button can flip without a round-trip.
  const [takenOver, setTakenOver] = useState(false)
  const [takeoverPending, setTakeoverPending] = useState(false)
  const [escalationPending, setEscalationPending] = useState(false)

  // Subscribe to the global `"browser-status"` Tauri event.
  // A single listener per BrowserCard instance; cleaned up on unmount or
  // when `sessionId` changes.
  //
  // `active` flag guards against the race where the async `listenToBrowserStatus`
  // resolves AFTER the cleanup function has already run (e.g. React Strict Mode
  // double-invoke). If the component is already unmounted by the time the Promise
  // resolves we call `fn()` immediately to unregister the listener.
  useEffect(() => {
    let unlisten: (() => void) | undefined
    let active = true

    listenToBrowserStatus((payload) => {
      // Filter to events for this card's session only.
      if (payload.session_id !== sessionId) return

      if (payload.running) {
        setBrowserStatus(sessionId, {
          running: true,
          url: payload.url,
          thumbnail: payload.thumbnail,
          backend: payload.backend ?? 'local_rust_cdp',
          title: payload.title ?? null,
          takenOver: payload.taken_over ?? false,
          lastAction: payload.last_action ?? null,
          diagnostics: {
            downloads: payload.downloads_count ?? 0,
            console: payload.console_count ?? 0,
            networkErrors: payload.network_error_count ?? 0,
          },
          escalationState: payload.escalation_state ?? 'none',
        })
      } else {
        // Browser stopped — clear the entry so the card disappears.
        clearBrowserSession(sessionId)
      }
    })
      .then((fn) => {
        if (active) {
          // Still mounted — store the unlisten function for cleanup.
          unlisten = fn
        } else {
          // Already unmounted — unregister immediately to avoid a leak.
          fn()
        }
      })
      .catch((err: unknown) => {
        console.error('[BrowserCard] Failed to subscribe to browser-status:', err)
      })

    return () => {
      active = false
      unlisten?.()
    }
  }, [sessionId])

  useEffect(() => {
    setTakenOver(entry?.takenOver ?? false)
  }, [entry?.takenOver])

  // Nothing to show when the browser is not running.
  if (!entry?.running) return null

  const hostname = hostnameOf(entry.url)
  const cockpitEntry = { ...entry, takenOver }

  const handleStop = (): void => {
    void (async () => {
      try {
        await closeBrowserSession(sessionId)
        clearBrowserSession(sessionId)
      } catch (err: unknown) {
        console.error('[BrowserCard] Failed to stop browser session:', err)
      }
    })()
  }

  const handleOpenViewer = (): void => {
    void openBrowserViewerWindow(sessionId).catch((err: unknown) => {
      console.error('[BrowserCard] Failed to open viewer window:', err)
    })
  }

  const handleToggleTakeover = (): void => {
    if (takeoverPending) return
    setTakeoverPending(true)
    void (async () => {
      try {
        if (takenOver) {
          await releaseBrowserTakeover(sessionId, true)
          setTakenOver(false)
          toast.success('已释放接管', {
            description: 'AI 浏览器已收回后台，工具调用已恢复。',
          })
        } else {
          await requestBrowserTakeover(sessionId)
          setTakenOver(true)
          toast.success('你已接管浏览器', {
            description: '一个真实的 Chrome 窗口已弹出；AI 工具调用暂停直到释放。',
          })
        }
      } catch (err: unknown) {
        toast.error(takenOver ? '释放接管失败' : '请求接管失败', {
          description: String(err),
        })
      } finally {
        setTakeoverPending(false)
      }
    })()
  }

  const handleRequestCloudEscalation = (): void => {
    if (escalationPending) return
    setEscalationPending(true)
    void (async () => {
      const reason = '用户从 Smart Browser cockpit 请求云端浏览器'
      try {
        await requestSmartBrowserCloudEscalation(sessionId, reason)
        const approved = window.confirm('是否批准将此浏览器会话升级到云端浏览器？')
        if (approved) {
          await approveSmartBrowserCloudEscalation(sessionId, reason)
          toast.success('已批准云端浏览器', {
            description: 'Smart Browser 已记录云端升级批准。',
          })
        } else {
          await denySmartBrowserCloudEscalation(sessionId, reason)
          toast.info('已保留本地浏览器', {
            description: '云端升级需要你的明确批准。',
          })
        }
      } catch (err: unknown) {
        toast.error('云端浏览器申请失败', {
          description: String(err),
        })
      } finally {
        setEscalationPending(false)
      }
    })()
  }

  return (
    <div
      className={cn(
        // Floating overlay — top-right corner of the chat area
        'absolute right-4 top-4 z-50',
        'w-[220px] overflow-hidden rounded-2xl',
        // Design-system surface: white/translucent with blur
        'border border-black/6 bg-white/90 shadow-[0_8px_28px_rgba(15,23,42,0.12)] backdrop-blur-[8px]',
      )}
      aria-label="AI 浏览器"
    >
      {/* ── Thumbnail ─────────────────────────────────────────────────────── */}
      <div className="relative overflow-hidden" style={{ aspectRatio: '16/9' }}>
        {entry.thumbnail ? (
          <img
            src={`data:image/jpeg;base64,${entry.thumbnail}`}
            alt="Browser viewport preview"
            // Prevent accidental drag that would disrupt window dragging.
            draggable={false}
            className="h-full w-full select-none object-cover"
          />
        ) : (
          <div className="flex h-full w-full items-center justify-center bg-muted">
            <Globe className="h-6 w-6 text-muted-foreground" />
          </div>
        )}

        {/* Animated "running" pill */}
        {!takenOver ? (
          <div className="absolute left-2 top-2 flex items-center gap-1.5 rounded-full bg-black/55 px-2 py-0.5 backdrop-blur-[4px]">
            {/* Jade pulse dot — uses the design-system --jade color token */}
            <span className="h-1.5 w-1.5 animate-pulse rounded-full bg-jade" />
            <span className="text-[10px] font-medium tracking-tight text-white/90">
              运行中
            </span>
          </div>
        ) : (
          // Phase 7C, slice 7C.3 — amber "user-taken-over" badge.
          <div className="absolute left-2 top-2 flex items-center gap-1.5 rounded-full bg-amber-500/90 px-2 py-0.5 backdrop-blur-[4px]">
            <Pause className="h-2.5 w-2.5 text-white" />
            <span className="text-[10px] font-medium tracking-tight text-white">
              用户接管中
            </span>
          </div>
        )}
      </div>

      {/* ── Info row ──────────────────────────────────────────────────────── */}
      <div className="flex items-center justify-between gap-2 px-3 py-2">
        {/* Hostname */}
        <div className="flex min-w-0 items-center gap-1.5">
          <Globe className="h-3 w-3 shrink-0 text-muted-foreground" />
          <span className="truncate text-[11px] text-foreground/65" title={entry.url ?? undefined}>
            {hostname}
          </span>
        </div>

        {/* Take over / release — Phase 7C, slice 7C.3 */}
        <button
          type="button"
          onClick={handleToggleTakeover}
          disabled={takeoverPending}
          className={cn(
            'flex h-6 w-6 shrink-0 items-center justify-center rounded-full',
            'transition-colors duration-150 active:scale-90',
            takenOver
              ? 'bg-amber-100 text-amber-700 hover:bg-amber-200'
              : 'text-muted-foreground hover:bg-black/8 hover:text-foreground',
            'disabled:cursor-wait disabled:opacity-50',
          )}
          aria-label={takenOver ? '释放接管' : '接管浏览器'}
          title={
            takenOver
              ? '释放接管 — 关闭可见窗口，AI 重新接手'
              : '接管浏览器 — 弹出真实 Chrome 窗口供你登录 / 解 CAPTCHA'
          }
        >
          <Hand className="h-3 w-3" />
        </button>

        {/* Open viewer window */}
        <button
          type="button"
          onClick={handleOpenViewer}
          disabled={takenOver}
          className={cn(
            'flex h-6 w-6 shrink-0 items-center justify-center rounded-full',
            'text-muted-foreground transition-colors duration-150',
            'hover:bg-black/8 hover:text-foreground',
            'active:scale-90 disabled:opacity-30 disabled:cursor-not-allowed',
          )}
          aria-label="查看浏览器"
          title={takenOver ? '接管中无法预览' : '查看浏览器'}
        >
          <Expand className="h-3 w-3" />
        </button>

        {/* Emergency stop */}
        <button
          type="button"
          onClick={handleStop}
          className={cn(
            'flex h-6 w-6 shrink-0 items-center justify-center rounded-full',
            'text-muted-foreground transition-colors duration-150',
            'hover:bg-destructive/10 hover:text-destructive',
            'active:scale-90',
          )}
          aria-label="停止浏览器"
          title="停止浏览器"
        >
          <Square className="h-3 w-3 fill-current" />
        </button>
      </div>
      <div className="border-t border-black/5 px-3 py-2">
        <SmartBrowserCockpit
          entry={cockpitEntry}
          compact
          escalationPending={escalationPending}
          onRequestCloudEscalation={handleRequestCloudEscalation}
        />
      </div>
    </div>
  )
}
