/**
 * BrowserViewerPage — toolbar for the AI Browser Viewer window.
 *
 * This component ONLY renders the thin toolbar at the top of the viewer window.
 * The live webpage content is displayed by a native WKWebView (macOS) / WebView2
 * (Windows) that the Rust backend attaches to the window area below this toolbar
 * via `Window::add_child`.  The native view fills the remaining height and renders
 * real pages — no iframe, no X-Frame-Options restrictions.
 *
 * The AI's navigation is mirrored into the native view by `sync_viewer_url()`
 * in `browser_tool.rs`.  The toolbar URL bar and back/forward/reload controls are
 * wired through the `navigate_viewer_window`, `browser_viewer_go_back`,
 * `browser_viewer_go_forward`, and `browser_viewer_reload` Tauri commands.
 */

import { useEffect, useRef, useState } from 'react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { ArrowLeft, ArrowRight, Globe, RefreshCw, Square, ExternalLink } from 'lucide-react'
import { cn } from '@/lib/utils'
import {
  closeBrowserSession,
  listenToBrowserStatus,
  requestBrowserStatus,
  type BrowserStatusEvent,
} from '@/lib/tauri'
import { invoke } from '@tauri-apps/api/core'
import { clearBrowserSession, setBrowserStatus } from '@/stores/browser-slice'

// ── Helpers ───────────────────────────────────────────────────────────────────

function hostnameOf(url: string | null): string {
  if (!url) return ''
  try {
    return new URL(url).hostname
  } catch {
    return url
  }
}

function openExternal(url: string): void {
  window.open(url, '_blank', 'noopener,noreferrer')
}

// ── Component ─────────────────────────────────────────────────────────────────

export function BrowserViewerPage() {
  const sessionId =
    new URLSearchParams(window.location.search).get('session_id') ?? ''

  const [running, setRunning] = useState(false)
  const [currentUrl, setCurrentUrl] = useState<string | null>(null)
  const [urlInput, setUrlInput] = useState('')

  const isDragging = useRef(false)

  // ── Bootstrap + live events ────────────────────────────────────────────────
  useEffect(() => {
    let unlisten: (() => void) | undefined
    let active = true

    // Ask backend for the current state immediately so the URL bar populates
    // without waiting for the next AI navigation.
    void requestBrowserStatus(sessionId).catch(() => {})

    listenToBrowserStatus((payload: BrowserStatusEvent) => {
      if (payload.session_id !== sessionId) return
      if (payload.running) {
        setRunning(true)
        setCurrentUrl(payload.url)
        setUrlInput(payload.url ?? '')
        setBrowserStatus(sessionId, {
          running: true,
          url: payload.url,
          thumbnail: payload.thumbnail,
        })
        // Mirror the current URL into the native WKWebView content area.
        // This is especially important when the viewer is opened after the AI
        // has already navigated — the content webview starts at about:blank and
        // needs to be told where to go.
        if (payload.url) {
          void invoke('navigate_viewer_window', {
            sessionId,
            url: payload.url,
          }).catch((err: unknown) => {
            console.warn('[BrowserViewerPage] navigate_viewer_window failed:', err)
          })
        }
      } else {
        setRunning(false)
        setCurrentUrl(null)
        setUrlInput('')
        clearBrowserSession(sessionId)
      }
    })
      .then((fn) => {
        if (active) unlisten = fn
        else fn()
      })
      .catch((err: unknown) => {
        console.error('[BrowserViewerPage] subscribe failed:', err)
      })

    return () => {
      active = false
      unlisten?.()
    }
  }, [sessionId])

  // ── Window drag ────────────────────────────────────────────────────────────
  const handleHeaderMouseDown = (e: React.MouseEvent<HTMLElement>): void => {
    if ((e.target as HTMLElement).closest('button,a,input')) return
    isDragging.current = true
    void getCurrentWindow()
      .startDragging()
      .catch(() => {})
      .finally(() => { isDragging.current = false })
  }

  // ── Toolbar actions ────────────────────────────────────────────────────────
  const handleNavigate = (url: string): void => {
    const normalized = url.startsWith('http://') || url.startsWith('https://')
      ? url
      : `https://${url}`
    void invoke('navigate_viewer_window', { sessionId, url: normalized }).catch(
      (err: unknown) => console.error('[viewer] navigate failed:', err),
    )
    setCurrentUrl(normalized)
    setUrlInput(normalized)
  }

  const handleUrlKeyDown = (e: React.KeyboardEvent<HTMLInputElement>): void => {
    if (e.key === 'Enter') handleNavigate(urlInput)
  }

  const handleGoBack = (): void => {
    void invoke('browser_viewer_go_back', { sessionId }).catch(() => {})
  }

  const handleGoForward = (): void => {
    void invoke('browser_viewer_go_forward', { sessionId }).catch(() => {})
  }

  const handleReload = (): void => {
    void invoke('browser_viewer_reload', { sessionId }).catch(() => {})
  }

  const handleStop = (): void => {
    void (async () => {
      try {
        await closeBrowserSession(sessionId)
        clearBrowserSession(sessionId)
      } catch {}
    })()
  }

  // ── Render ─────────────────────────────────────────────────────────────────
  // This component renders ONLY the toolbar.
  // The native WKWebView content area is added by Rust (Window::add_child) and
  // sits below this component — it is NOT a DOM element we can control here.

  return (
    <div
      className="flex h-screen flex-col overflow-hidden select-none bg-[#f5f6f7]"
      style={{ maxHeight: '52px' }}   // toolbar only — content view fills the rest natively
    >
      <header
        data-tauri-drag-region
        className="flex h-[52px] shrink-0 items-center gap-2 border-b border-black/8 bg-white/90 px-4 backdrop-blur-[8px]"
        onMouseDown={handleHeaderMouseDown}
      >
        {/* Traffic-light spacer */}
        <div className="w-[68px] shrink-0" />

        {/* Back / Forward / Reload */}
        <div
          className="flex shrink-0 items-center gap-0.5"
          onMouseDown={(e) => e.stopPropagation()}
        >
          <button
            type="button"
            onClick={handleGoBack}
            className={cn(
              'flex h-7 w-7 items-center justify-center rounded-md',
              'text-muted-foreground transition-colors hover:bg-black/6 hover:text-foreground',
            )}
            title="后退"
          >
            <ArrowLeft className="h-3.5 w-3.5" />
          </button>
          <button
            type="button"
            onClick={handleGoForward}
            className={cn(
              'flex h-7 w-7 items-center justify-center rounded-md',
              'text-muted-foreground transition-colors hover:bg-black/6 hover:text-foreground',
            )}
            title="前进"
          >
            <ArrowRight className="h-3.5 w-3.5" />
          </button>
          <button
            type="button"
            onClick={handleReload}
            className={cn(
              'flex h-7 w-7 items-center justify-center rounded-md',
              'text-muted-foreground transition-colors hover:bg-black/6 hover:text-foreground',
            )}
            title="刷新"
          >
            <RefreshCw className="h-3.5 w-3.5" />
          </button>
        </div>

        {/* URL bar */}
        <div
          className="flex min-w-0 flex-1 items-center gap-1.5 rounded-lg bg-black/5 px-2.5 py-1"
          onMouseDown={(e) => e.stopPropagation()}
        >
          <Globe className="h-3 w-3 shrink-0 text-muted-foreground/50" />
          <input
            type="text"
            value={urlInput}
            onChange={(e) => setUrlInput(e.target.value)}
            onKeyDown={handleUrlKeyDown}
            onFocus={(e) => e.target.select()}
            placeholder={running ? '正在加载…' : '等待 AI 导航'}
            className={cn(
              'min-w-0 flex-1 bg-transparent text-[12px] font-mono',
              'text-foreground/70 placeholder:text-muted-foreground/50',
              'outline-none',
            )}
            spellCheck={false}
          />
        </div>

        {/* Right actions */}
        <div
          className="flex shrink-0 items-center gap-1"
          onMouseDown={(e) => e.stopPropagation()}
        >
          {/* Running indicator */}
          {running && (
            <div className="flex items-center gap-1.5 px-2 text-[11px] text-jade/80">
              <span className="h-1.5 w-1.5 animate-pulse rounded-full bg-jade" />
              <span>AI 操作中</span>
            </div>
          )}

          {/* Open in system browser */}
          {currentUrl && (
            <button
              type="button"
              onClick={() => openExternal(currentUrl)}
              className={cn(
                'flex h-7 items-center gap-1 rounded-lg px-2',
                'text-[11px] text-muted-foreground transition-colors hover:bg-black/6 hover:text-foreground',
              )}
              title="在默认浏览器中打开"
            >
              <ExternalLink className="h-3 w-3" />
            </button>
          )}

          {/* Stop AI browser */}
          {running && (
            <button
              type="button"
              onClick={handleStop}
              className={cn(
                'flex h-7 items-center gap-1.5 rounded-lg px-2.5',
                'text-[11px] text-destructive/70 transition-colors hover:bg-destructive/8 hover:text-destructive',
              )}
              title="停止 AI 浏览器"
            >
              <Square className="h-3 w-3 fill-current" />
              <span>停止</span>
            </button>
          )}
        </div>
      </header>
    </div>
  )
}
