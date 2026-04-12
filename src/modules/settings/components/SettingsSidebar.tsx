import { Settings } from 'lucide-react'
import { SETTINGS_SECTIONS } from '../data'
import type { SettingsSectionId } from '../types'
import { SettingsSidebarItem } from './SettingsSidebarItem'

interface SettingsSidebarProps {
  activeSection: SettingsSectionId
  onSectionChange: (section: SettingsSectionId) => void
}

export function SettingsSidebar({ activeSection, onSectionChange }: SettingsSidebarProps) {
  return (
    <aside className="flex w-[304px] shrink-0 flex-col border-r border-black/5 bg-[#eef0f1]/56 px-3 py-5 select-none backdrop-blur-[2px]">
      <div className="mb-4 px-1">
        <div className="flex items-center gap-3">
          <div className="flex size-8 items-center justify-center rounded-[14px] bg-black/5 text-black/70">
            <Settings className="h-4 w-4" />
          </div>
          <div>
            <div className="text-[22px] font-semibold tracking-tight text-foreground">设置</div>
            <div className="text-[12px] leading-5 text-muted-foreground">统一管理 If2Ai 的外观、权限和连接能力。</div>
          </div>
        </div>
      </div>

      <nav className="flex-1">
        {SETTINGS_SECTIONS.map((section) => (
          <SettingsSidebarItem
            key={section.id}
            icon={section.icon}
            label={section.label}
            active={section.id === activeSection}
            onClick={() => onSectionChange(section.id)}
          />
        ))}
      </nav>
    </aside>
  )
}
