import { useEffect, useState } from 'react'
import { toast } from 'sonner'
import { getDaydreamConfig, setDaydreamConfig } from '@/api/memory'
import type { DaydreamConfig } from '@/transport/contracts'
import { DaydreamStatusRow } from './DaydreamStatusRow'

const STRATEGIES: { value: DaydreamConfig['strategy']; label: string }[] = [
  { value: 'conservative', label: 'Conservative — prune only' },
  { value: 'balanced', label: 'Balanced — prune + merge + reflect (recommended)' },
  { value: 'aggressive', label: 'Aggressive — adds refresh' },
]

/// Settings section for the daydream background consolidation feature.
/// Loads config on mount and persists patches on change.
export function DaydreamSettingsSection() {
  const [config, setConfig] = useState<DaydreamConfig | null>(null)

  useEffect(() => {
    getDaydreamConfig()
      .then(setConfig)
      .catch((err) => toast.error(`Failed to load daydream config: ${err}`))
  }, [])

  const update = (patch: Partial<DaydreamConfig>) => {
    if (!config) return
    const next = { ...config, ...patch }
    setConfig(next)
    setDaydreamConfig(next).catch((err) => toast.error(`Failed to save: ${err}`))
  }

  if (!config) {
    return null
  }

  return (
    <section className="rounded-lg border border-border bg-card p-4 space-y-3">
      <header>
        <h3 className="text-sm font-semibold">Daydream consolidation</h3>
        <p className="text-xs text-muted-foreground mt-1">
          Background memory pruning + dedup that runs after a period of inactivity. Off by default;
          changes take effect at next launch.
        </p>
      </header>

      <label className="flex items-center gap-2 text-sm cursor-pointer">
        <input
          type="checkbox"
          checked={config.enabled}
          onChange={(e) => update({ enabled: e.target.checked })}
        />
        Enable background consolidation
      </label>

      <label className="block text-sm">
        <span className="block mb-1 text-muted-foreground">Strategy</span>
        <select
          value={config.strategy}
          onChange={(e) =>
            update({ strategy: e.target.value as DaydreamConfig['strategy'] })
          }
          className="w-full px-2 py-1.5 rounded-md border border-border bg-background text-sm"
        >
          {STRATEGIES.map((s) => (
            <option key={s.value} value={s.value}>
              {s.label}
            </option>
          ))}
        </select>
      </label>

      <DaydreamStatusRow />
    </section>
  )
}
