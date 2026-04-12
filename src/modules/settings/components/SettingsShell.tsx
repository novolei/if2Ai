import type { ReactNode } from 'react'
import { Settings, X } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { SETTINGS_SECTIONS } from '../data'
import type { SettingsSectionId } from '../types'
import { SettingsSidebar } from './SettingsSidebar'

interface SettingsShellProps {
  activeSection: SettingsSectionId
  onSectionChange: (section: SettingsSectionId) => void
  onClose: () => void
  children: ReactNode
}

export function SettingsShell({
  activeSection,
  onSectionChange,
  onClose,
  children,
}: SettingsShellProps) {
  const section = SETTINGS_SECTIONS.find((item) => item.id === activeSection) ?? SETTINGS_SECTIONS[0]

  return (
    <div className="relative isolate flex h-screen overflow-hidden bg-transparent text-foreground">
      <div className="relative z-10 flex h-full min-h-0 w-full flex-col">
        <header className="window-drag flex h-14 shrink-0 items-center justify-between border-b border-black/5 bg-[#f2f3f4]/56 px-4 select-none backdrop-blur-[2px] lg:px-5">
          <div className="flex items-center gap-3">
            <div className="flex size-9 items-center justify-center rounded-[14px] bg-black/5 text-black/70">
              <Settings className="h-4 w-4" />
            </div>
            <div>
              <div className="text-sm font-semibold tracking-tight">设置</div>
              <div className="text-xs text-muted-foreground">管理 If2Ai 的桌面体验</div>
            </div>
          </div>

          <Button
            type="button"
            size="icon"
            variant="ghost"
            onClick={onClose}
            className="window-no-drag h-9 w-9 rounded-full text-muted-foreground hover:bg-white/70 hover:text-foreground"
          >
            <X className="h-4 w-4" />
          </Button>
        </header>

        <div className="flex min-h-0 flex-1">
          <SettingsSidebar activeSection={activeSection} onSectionChange={onSectionChange} />

          <main className="min-w-0 flex-1 overflow-y-auto px-5 py-5 lg:px-8 lg:py-6">
            <div className="flex w-full flex-col gap-5">
              <div className="space-y-2">
                <div className="text-[32px] font-semibold tracking-tight text-foreground">
                  {section.label}
                </div>
                <p className="max-w-3xl text-[13px] leading-6 text-muted-foreground">
                  {section.description}
                </p>
              </div>

              {children}
            </div>
          </main>
        </div>
      </div>
    </div>
  )
}
