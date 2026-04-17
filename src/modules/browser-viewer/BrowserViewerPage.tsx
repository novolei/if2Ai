/**
 * BrowserViewerPage — AI Browser Viewer standalone window.
 *
 * Loaded by the Tauri `browser-viewer-*` windows via
 * `index.html?window=browser-viewer&session_id=<id>`.
 *
 * Displays the current URL and live thumbnail that the AI agent is operating.
 * The page also embeds an iframe for the URL so the user can observe the
 * page content (iframe rendering is best-effort — some sites block embedding
 * via X-Frame-Options or CSP frame-ancestors; the thumbnail is always shown
 * as a reliable fallback).
 *
 * **Isolation note**: This window navigates to the same URL independently
 * and does NOT share cookies, localStorage, or state with the headless
 * chromiumoxide session the AI is using.
 */

import { useEffect, useState } from 'react'
import { Globe, ExternalLink, RefreshCw, Square, AlertCircle } from 'lucide-react'
import { cn } from '@/lib/utils'
import {
  closeBrowserSession,
  listenToBrowserStatus,
  type BrowserStatusEvent,
} from '@/lib/tauri'
import { clearBrowserSession, setBrowserStatus } from '@/stores/browser-slice'

// ── Helpers ───────────────────────────────────────────────────────────────────

function hostnameOf(url: string | null): string {
  if (!url) return '—'
  try {
    return new URL(url).hostname
  } catch {
    return url
  }
}

function openExternal(url: string): void {
  // Use window.open in a Tauri WebviewWindow context.
  window.open(url, '_blank', 'noopener,noreferrer')
}

// ── Component ─────────────────────────────────────────────────────────────────

interface ViewerState {
  running: boolean
  url: string | null
  thumbnail: string | null
}

/**
 * Full-page component rendered inside the `browser-viewer-*` Tauri window.
 * Reads `session_id` from the URL query string.
 */
export function BrowserViewerPage() {
  // Read session_id from query string — injected by open_browser_viewer_window.
  const sessionId =
    new URLSearchParams(window.location.search).get('session_id') ?? ''

  const [state, setState] = useState<ViewerState>({
    running: false,
    url: null,
    thumbnail: null,
  })
  const [iframeKey, setIframeKey] = useState(0)
  const [iframeBlocked, setIframeBlocked] = useState(false)

  // Subscribe to browser-status events with proper cleanup (active-flag pattern
  // guards against the race where the async listener resolves after unmount).
  useEffect(() => {
    let unlisten: (() => void) | undefined
    let active = true

    listenToBrowserStatus((payload: BrowserStatusEvent) => {
      if (payload.session_id !== sessionId) return

      if (payload.running) {
        setState({
          running: true,
          url: payload.url,
          thumbnail: payload.thumbnail,
        })
        setBrowserStatus(sessionId, {
          running: true,
          url: payload.url,
          thumbnail: payload.thumbnail,
        })
        // Reset iframe block flag when URL changes.
        setIframeBlocked(false)
      } else {
        setState({ running: false, url: null, thumbnail: null })
        clearBrowserSession(sessionId)
      }
    })
      .then((fn) => {
        if (active) {
          unlisten = fn
        } else {
          fn()
        }
      })
      .catch((err: unknown) => {
        console.error('[BrowserViewerPage] Failed to subscribe:', err)
      })

    return () => {
      active = false
      unlisten?.()
    }
  }, [sessionId])

  const handleStop = (): void => {
    void (async () => {
      try {
        await closeBrowserSession(sessionId)
        clearBrowserSession(sessionId)
      } catch (err: unknown) {
        console.error('[BrowserViewerPage] stop failed:', err)
      }
    })()
  }

  const handleRefresh = (): void => {
    setIframeBlocked(false)
    setIframeKey((k) => k + 1)
  }

  // ── Render ───────────────────────────────────────────────────────────────────

  return (
    <div className="flex h-screen flex-col overflow-hidden bg-[#f6f7f8] select-none">
      {/* ── Toolbar ─────────────────────────────────────────────────────────── */}
      <header
        className="window-drag flex h-12 shrink-0 items-center gap-3 border-b border-black/5 bg-[#eef0f1]/88 px-4 backdrop-blur-[6px]"
        // Allow native window drag from the header
        onMouseDown={(e) => {
          // Let button clicks pass through without triggering drag
          if ((e.target as HTMLElement).closest('button,a')) return
        }}
      >
        {/* Traffic-light area spacer (macOS) */}
        <div className="w-[72px] shrink-0" />

        {/* URL display */}
        <div className="flex min-w-0 flex-1 items-center gap-2">
          <Globe className="h-4 w-4 shrink-0 text-muted-foreground" />
          <span
            className="window-no-drag min-w-0 truncate text-[13px] text-foreground/70"
            data-window-no-drag="true"
            title={state.url ?? undefined}
          >
            {state.url ?? (state.running ? '導航中…' : '等待 AI 使用浏览器')}
          </span>
        </div>

        {/* Action buttons */}
        <div
          className="window-no-drag flex shrink-0 items-center gap-1"
          data-window-no-drag="true"
        >
          {/* Refresh iframe */}
          <button
            type="button"
            onClick={handleRefresh}
            disabled={!state.url}
            className={cn(
              'flex h-7 w-7 items-center justify-center rounded-lg',
              'text-muted-foreground transition-colors',
              'hover:bg-black/6 hover:text-foreground',
              'disabled:opacity-40',
            )}
            title="刷新预览"
          >
            <RefreshCw className="h-3.5 w-3.5" />
          </button>

          {/* Open in system browser */}
          {state.url && (
            <button
              type="button"
              onClick={() => openExternal(state.url!)}
              className={cn(
                'flex h-7 items-center gap-1.5 rounded-lg px-2.5',
                'text-[12px] text-muted-foreground transition-colors',
                'hover:bg-black/6 hover:text-foreground',
              )}
              title="在系统浏览器中打开"
            >
              <ExternalLink className="h-3 w-3" />
              <span>在浏览器中打开</span>
            </button>
          )}

          {/* Emergency stop */}
          {state.running && (
            <button
              type="button"
              onClick={handleStop}
              className={cn(
                'flex h-7 items-center gap-1.5 rounded-lg px-2.5',
                'text-[12px] transition-colors',
                'text-destructive/70 hover:bg-destructive/8 hover:text-destructive',
              )}
              title="停止 AI 浏览器"
            >
              <Square className="h-3 w-3 fill-current" />
              <span>停止</span>
            </button>
          )}
        </div>
      </header>

      {/* ── Isolation notice ─────────────────────────────────────────────────── */}
      <div className="flex shrink-0 items-center gap-2 border-b border-black/4 bg-jade/6 px-4 py-1.5">
        <AlertCircle className="h-3 w-3 shrink-0 text-jade" />
        <span className="text-[11px] text-jade/80">
          此窗口独立浏览，不共享 AI 会话的 cookies 或登录状态
        </span>
        {state.running && (
          <span className="ml-auto flex items-center gap-1 text-[11px] text-jade">
            <span className="h-1.5 w-1.5 animate-pulse rounded-full bg-jade" />
            AI 正在操作
          </span>
        )}
      </div>

      {/* ── Main content ─────────────────────────────────────────────────────── */}
      <div className="relative min-h-0 flex-1 overflow-hidden">
        {state.url ? (
          <>
            {/* Embedded iframe — best-effort; some sites block via X-Frame-Options */}
            {!iframeBlocked && (
              <iframe
                key={iframeKey}
                src={state.url}
                title="AI Browser Viewer"
                className="absolute inset-0 h-full w-full border-0 block"
                sandbox="allow-same-origin allow-scripts allow-forms allow-popups"
                onError={() => setIframeBlocked(true)}
                // Detect if iframe load was blocked (heuristic via load with empty doc)
                onLoad={(e) => {
                  try {
                    const doc = (e.target as HTMLIFrameElement).contentDocument
                    if (doc && doc.body && doc.body.innerHTML === '') {
                      setIframeBlocked(true)
                    }
                  } catch {
                    // Cross-origin: can't read content, assume it loaded.
                  }
                }}
              />
            )}

            {/* Fallback: show thumbnail when iframe is blocked */}
            {iframeBlocked && (
              <div className="flex h-full flex-col items-center justify-center gap-6 p-8">
                {state.thumbnail ? (
                  <div className="overflow-hidden rounded-2xl border border-black/8 shadow-[0_12px_40px_rgba(15,23,42,0.12)]">
                    <img
                      src={`data:image/jpeg;base64,${state.thumbnail}`}
                      alt="Page screenshot"
                      draggable={false}
                      className="block max-h-[60vh] max-w-full select-none object-contain"
                    />
                  </div>
                ) : (
                  <div className="flex h-40 w-80 items-center justify-center rounded-2xl bg-muted">
                    <Globe className="h-10 w-10 text-muted-foreground/40" />
                  </div>
                )}
                <div className="text-center">
                  <p className="text-[13px] font-medium text-foreground/70">
                    {hostnameOf(state.url)}
                  </p>
                  <p className="mt-1 text-[12px] text-muted-foreground">
                    此网站不允许嵌入预览
                  </p>
                  <button
                    type="button"
                    onClick={() => openExternal(state.url!)}
                    className="mt-3 flex items-center gap-1.5 rounded-full border border-black/10 bg-white px-4 py-1.5 text-[12px] text-foreground/70 shadow-sm hover:bg-black/4"
                  >
                    <ExternalLink className="h-3 w-3" />
                    在浏览器中打开
                  </button>
                </div>
              </div>
            )}
          </>
        ) : (
          /* No URL yet — placeholder */
          <div className="flex h-full flex-col items-center justify-center gap-4 text-center">
            <div className="flex h-16 w-16 items-center justify-center rounded-2xl bg-muted">
              <Globe className="h-7 w-7 text-muted-foreground/50" />
            </div>
            <div>
              <p className="text-[14px] font-medium text-foreground/60">
                等待 AI 使用浏览器
              </p>
              <p className="mt-1 text-[12px] text-muted-foreground">
                当 AI 导航到页面时，这里将显示预览
              </p>
            </div>
          </div>
        )}
      </div>
    </div>
  )
}
