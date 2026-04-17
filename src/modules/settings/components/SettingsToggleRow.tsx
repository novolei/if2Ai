import { Switch } from '@/components/ui/switch'
import { SettingsRow } from './SettingsRow'

interface SettingsToggleRowProps {
  title: string
  description?: string
  checked: boolean
  onCheckedChange: (checked: boolean) => void
  /** When true, renders as a borderless inline row (inside a surface divider list) */
  inline?: boolean
}

export function SettingsToggleRow({
  title,
  description,
  checked,
  onCheckedChange,
  inline,
}: SettingsToggleRowProps) {
  return (
    <SettingsRow title={title} description={description} inline={inline}>
      <Switch checked={checked} onCheckedChange={onCheckedChange} />
    </SettingsRow>
  )
}
