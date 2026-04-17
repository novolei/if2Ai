import { Sparkles, X } from 'lucide-react'
import { SETTINGS_SECTIONS } from '../data'
import type { SettingsSectionId } from '../types'
import { SettingsSidebarItem } from './SettingsSidebarItem'

interface SettingsSidebarProps {
  activeSection: SettingsSectionId
  onSectionChange: (section: SettingsSectionId) => void
  onClose: () => void
}

export function SettingsSidebar({ activeSection, onSectionChange, onClose }: SettingsSidebarProps) {
  return (
    <aside className="flex w-[200px] shrink-0 flex-col border-r border-black/[0.07] bg-[#f0f1f2] px-3 py-3 select-none">
      {/* ── Logo + close ── */}
      <div className="window-drag mb-4 flex items-center justify-between px-1 pt-1">
        <div className="flex items-center gap-2">
          <div
            className="flex size-6 items-center justify-center rounded-[7px] bg-jade"
            style={{ boxShadow: '0 1.5px 4px color-mix(in oklch, var(--jade) 40%, transparent)' }}
          >
            <Sparkles className="size-3 text-white" strokeWidth={1.8} />
          </div>
          <div>
            <div className="text-[12.5px] font-bold tracking-tight text-foreground/80">If2Ai</div>
            <div className="text-[9.5px] font-semibold uppercase tracking-[0.12em] text-black/25">
              设置
            </div>
          </div>
        </div>

        <button
          type="button"
          onClick={onClose}
          className="window-no-drag flex size-6 items-center justify-center rounded-lg text-black/30 transition-colors hover:bg-black/[0.07] hover:text-black/60"
          aria-label="关闭设置"
        >
          <X className="size-3.5" />
        </button>
      </div>

      {/* ── Nav ── */}
      <nav className="flex flex-1 flex-col gap-0.5">
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
