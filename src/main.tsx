import { createRoot } from 'react-dom/client'
import '@fontsource-variable/geist'
import './styles/globals.css'
import App from './App'
import { SettingsApp } from './components/settings/SettingsApp'
import { If2AiLoadingScreen } from './components/loading/If2AiLoadingScreen'
import { closeSettingsWindow } from '@/lib/tauri'
import { TooltipProvider } from '@/components/ui/tooltip'

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
  return <App />
}

createRoot(document.getElementById('app')!).render(
  <TooltipProvider>
    <Root />
  </TooltipProvider>,
)
