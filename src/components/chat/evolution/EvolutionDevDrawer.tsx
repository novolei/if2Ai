// UI-001 — Top-level Tab container for the 6 Agent Evolution dev panels.
//
// Pure presentational React shell:
//   - No IPC calls; only `useEvolutionEventSelector` reads.
//   - Hidden by default; opt-in via env (`IF2AI_DEV_EVOLUTION_UI=true`)
//     or the `devMode` prop wired by `TelemetryDrawer`.
//   - Each tab body is a separate UI-002~006 component; this file is
//     deliberately a layout-only shell.

import { useState } from 'react'

import { DaemonHealthDashboard } from './DaemonHealthDashboard.tsx'
import { CompressionHistoryTable } from './CompressionHistoryTable.tsx'
import { SkillSedimentationTimeline } from './SkillSedimentationTimeline.tsx'
import { SelfEditPanel } from './SelfEditPanel.tsx'
import { EvolutionMiscPanel } from './EvolutionMiscPanel.tsx'

export type EvolutionTabId =
  | 'daemon'
  | 'compression'
  | 'skills'
  | 'self_edit'
  | 'misc'

export interface EvolutionTabSpec {
  readonly id: EvolutionTabId
  readonly label: string
}

export const EVOLUTION_TABS: readonly EvolutionTabSpec[] = [
  { id: 'daemon', label: 'Daemon Health' },
  { id: 'compression', label: 'Compression' },
  { id: 'skills', label: 'Skills' },
  { id: 'self_edit', label: 'Self Edit' },
  { id: 'misc', label: 'Browser/DK/Checkpoint' },
] as const

export interface EvolutionDevDrawerProps {
  /** Override the env-based gate. Defaults to checking
   * `import.meta.env.IF2AI_DEV_EVOLUTION_UI === 'true'`. */
  readonly devMode?: boolean
}

function isDevModeEnvOn(): boolean {
  // Vite injects env vars under `import.meta.env`. Defensive guard
  // for non-Vite test runners.
  try {
    const meta = import.meta as unknown as { env?: Record<string, string | undefined> }
    return meta.env?.IF2AI_DEV_EVOLUTION_UI === 'true'
  } catch {
    return false
  }
}

export function EvolutionDevDrawer({ devMode }: EvolutionDevDrawerProps) {
  const enabled = devMode ?? isDevModeEnvOn()
  const [activeTab, setActiveTab] = useState<EvolutionTabId>('daemon')
  if (!enabled) return null

  return (
    <section
      data-testid="evolution-dev-drawer"
      className="rounded-md border border-zinc-200 bg-white/70 p-3 text-xs text-zinc-700 shadow-sm"
    >
      <header className="mb-2 flex items-center gap-2">
        <h3 className="text-sm font-medium text-zinc-900">Agent Evolution (dev)</h3>
        <span className="text-[10px] uppercase tracking-wider text-zinc-400">UI-001</span>
      </header>
      <nav className="mb-2 flex flex-wrap gap-1">
        {EVOLUTION_TABS.map((tab) => (
          <button
            key={tab.id}
            type="button"
            data-testid={`evolution-tab-${tab.id}`}
            data-active={activeTab === tab.id}
            onClick={() => setActiveTab(tab.id)}
            className={
              activeTab === tab.id
                ? 'rounded bg-zinc-900 px-2 py-1 text-white'
                : 'rounded bg-zinc-100 px-2 py-1 text-zinc-700 hover:bg-zinc-200'
            }
          >
            {tab.label}
          </button>
        ))}
      </nav>
      <div className="mt-2">
        {activeTab === 'daemon' && <DaemonHealthDashboard />}
        {activeTab === 'compression' && <CompressionHistoryTable />}
        {activeTab === 'skills' && <SkillSedimentationTimeline />}
        {activeTab === 'self_edit' && <SelfEditPanel />}
        {activeTab === 'misc' && <EvolutionMiscPanel />}
      </div>
    </section>
  )
}
