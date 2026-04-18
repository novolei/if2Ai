import { createRoot } from 'react-dom/client'
import '@fontsource-variable/geist'
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

  return <App />
}

createRoot(document.getElementById('app')!).render(
  <ThemeProvider>
    <TooltipProvider>
      <Root />
    </TooltipProvider>
  </ThemeProvider>,
)
