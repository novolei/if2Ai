import type { ReactNode } from 'react'
import { Toaster } from '@/components/ui/sonner'
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
  const Icon = section.icon

  return (
    <div className="relative isolate flex h-screen overflow-hidden bg-transparent text-foreground">
      <div className="relative z-10 flex h-full min-h-0 w-full">
        {/* Sidebar */}
        <SettingsSidebar
          activeSection={activeSection}
          onSectionChange={onSectionChange}
          onClose={onClose}
        />

        {/* Content */}
        <main className="flex min-h-0 flex-1 flex-col overflow-y-auto bg-[#f5f6f7] px-6 py-5 lg:px-8 lg:py-6">
          {/* Page header */}
          <div className="mb-5 flex items-center gap-3">
            <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-xl bg-jade/10">
              <Icon className="h-4 w-4 text-jade" />
            </div>
            <div>
              <div className="text-[16px] font-semibold tracking-tight">{section.label}</div>
              <p className="text-[11.5px] leading-4 text-muted-foreground">{section.description}</p>
            </div>
          </div>

          {children}
        </main>
      </div>

      <Toaster position="top-right" richColors closeButton />
    </div>
  )
}
