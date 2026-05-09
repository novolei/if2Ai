import { useState } from 'react'
import { useProceduralEntries } from './use-procedural-entries'
import type { ProceduralEntryDto } from '@/transport/contracts'

const CATEGORY_LABEL: Record<string, string> = {
  HeuristicRule: 'Heuristic',
  AntiPattern: 'Anti-pattern',
  BestPractice: 'Best practice',
  UserPreference: 'User preference',
  ToolUsagePattern: 'Tool usage',
}

function formatTrust(t: number): string {
  if (t >= 0.85) return 'high'
  if (t >= 0.5) return 'med'
  return 'low'
}

function ProcedureRow({ entry }: { entry: ProceduralEntryDto }) {
  const [open, setOpen] = useState(false)
  const label = CATEGORY_LABEL[entry.category] ?? entry.category
  return (
    <li className="border border-border rounded-md p-2">
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        className="w-full text-left text-sm flex items-center gap-2"
      >
        <span className="px-1.5 py-0.5 rounded bg-muted text-xs">{label}</span>
        <span className="flex-1 truncate">{entry.content}</span>
        <span className="text-xs text-muted-foreground">trust {formatTrust(entry.trustScore)}</span>
      </button>
      {open && (
        <div className="mt-2 text-xs text-muted-foreground space-y-1">
          <div>
            Key: <code className="break-all">{entry.key}</code>
          </div>
          <div>
            Trust score: {entry.trustScore.toFixed(2)} · Access count: {entry.accessCount}
          </div>
          <div>Updated: {new Date(entry.updatedAt).toLocaleString()}</div>
          <div className="pt-1">{entry.content}</div>
        </div>
      )}
    </li>
  )
}

export function LearnedProceduresPanel() {
  const { entries, refresh } = useProceduralEntries()

  if (entries === null) {
    return <p className="text-xs text-muted-foreground">Loading…</p>
  }
  if (entries.length === 0) {
    return (
      <p className="text-xs text-muted-foreground">
        No procedures learned yet. Run a few chat turns with daydream enabled, then trigger a cycle.
        Failure runs and repeat tool patterns are most likely to produce insights.
      </p>
    )
  }

  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between">
        <h5 className="text-xs font-medium text-muted-foreground uppercase tracking-wide">
          Learned procedures ({entries.length})
        </h5>
        <button
          type="button"
          onClick={() => void refresh()}
          className="text-xs text-muted-foreground hover:text-foreground"
        >
          Refresh
        </button>
      </div>
      <ul className="space-y-1.5 max-h-64 overflow-auto">
        {entries.map((e) => (
          <ProcedureRow key={e.key} entry={e} />
        ))}
      </ul>
    </div>
  )
}
