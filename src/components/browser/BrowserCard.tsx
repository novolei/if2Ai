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

import { useEffect } from 'react'
import { Globe, Square } from 'lucide-react'
import { cn } from '@/lib/utils'
import {
  closeBrowserSession,
  listenToBrowserStatus,
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

  // Nothing to show when the browser is not running.
  if (!entry?.running) return null

  const hostname = hostnameOf(entry.url)

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
        <div className="absolute left-2 top-2 flex items-center gap-1.5 rounded-full bg-black/55 px-2 py-0.5 backdrop-blur-[4px]">
          {/* Jade pulse dot — uses the design-system --jade color token */}
          <span className="h-1.5 w-1.5 animate-pulse rounded-full bg-jade" />
          <span className="text-[10px] font-medium tracking-tight text-white/90">
            运行中
          </span>
        </div>
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
    </div>
  )
}
