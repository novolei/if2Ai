import { Switch } from '@/components/ui/switch'
import { SettingsRow } from './SettingsRow'

interface SettingsToggleRowProps {
  title: string
  description?: string
  checked: boolean
  onCheckedChange: (checked: boolean) => void
}

export function SettingsToggleRow({ title, description, checked, onCheckedChange }: SettingsToggleRowProps) {
  return (
    <SettingsRow title={title} description={description}>
      <div className="flex items-center justify-end">
        <Switch checked={checked} onCheckedChange={onCheckedChange} />
      </div>
    </SettingsRow>
  )
}

