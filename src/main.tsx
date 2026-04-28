import { createRoot } from 'react-dom/client'
import '@fontsource-variable/geist'
import '@fontsource/noto-emoji/400.css'
import '@fontsource/noto-color-emoji/400.css'
import './styles/globals.css'
import App from './App'
import { SettingsApp } from './components/settings/SettingsApp'
import { If2AiLoadingScreen } from './components/loading/If2AiLoadingScreen'
import { BrowserViewerPage } from './modules/browser-viewer/BrowserViewerPage'
import { closeSettingsWindow } from '@/lib/tauri'
import { TooltipProvider } from '@/components/ui/tooltip'
import { ThemeProvider } from '@/components/theme/ThemeProvider'

const urlParams = new URLSearchParams(window.location.search)
const windowType = urlParams.get('window')
const demoType = urlParams.get('demo')

function hasTauriRuntime(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
}

function BrowserRuntimeDiagnostic() {
  return (
    <main className="flex min-h-screen items-center justify-center bg-background px-6 text-foreground">
      <section className="w-full max-w-lg rounded-xl border border-border/70 bg-card px-6 py-5 shadow-sm">
        <div className="text-[11px] font-semibold uppercase tracking-widest text-muted-foreground">
          Developer runtime diagnostic
        </div>
        <h1 className="mt-2 text-[18px] font-semibold tracking-tight">
          This URL is the Vite dev server
        </h1>
        <p className="mt-3 text-[13px] leading-6 text-muted-foreground">
          If2Ai needs the Tauri runtime bridge for IPC, windows, local files,
          activation, projects, and sessions. A regular browser at
          <code className="mx-1 rounded bg-muted px-1 py-0.5 font-mono text-[12px]">
            localhost:9527
          </code>
          cannot provide that bridge, so the full app shell is not mounted here.
        </p>
        <div className="mt-4 rounded-lg border border-border/70 bg-muted/40 px-3 py-2 text-[12px] leading-5 text-muted-foreground">
          Keep <code className="font-mono">npx tauri dev</code> running and use
          the native If2Ai window it opens. Browser-only demos can still use
          query routes such as <code className="font-mono">?demo=loading</code>.
        </div>
      </section>
    </main>
  )
}

const Root = () => {
  if (demoType === 'loading') {
    return (
      <If2AiLoadingScreen
        projectName={urlParams.get('project') || 'If2Ai'}
        stageLabel={urlParams.get('stage') || 'Initializing agent workspace'}
      />
    )
  }

  if (windowType === 'settings') {
    return <SettingsApp onClose={closeSettingsWindow} />
  }

  if (windowType === 'browser-viewer') {
    return <BrowserViewerPage />
  }

  if (!hasTauriRuntime()) {
    return <BrowserRuntimeDiagnostic />
  }

  return <App />
}

createRoot(document.getElementById('app')!).render(
  <ThemeProvider>
    <TooltipProvider>
      <Root />
    </TooltipProvider>
  </ThemeProvider>,
)
