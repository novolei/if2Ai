import { createRoot } from 'react-dom/client'
import '@fontsource-variable/geist'
import './styles/globals.css'
import App from './App'
import { SettingsApp } from './components/settings/SettingsApp'
import { closeSettingsWindow } from '@/lib/tauri'

const urlParams = new URLSearchParams(window.location.search)
const windowType = urlParams.get('window')

const Root = () => {
  if (windowType === 'settings') {
    return <SettingsApp onClose={closeSettingsWindow} />
  }
  return <App />
}

createRoot(document.getElementById('app')!).render(
  <Root />,
)
