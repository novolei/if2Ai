// UI-002 — Daemon health dashboard.
// Reads `daemon_health` ring buffer; renders a minimal table.

import { useEvolutionEventSelector } from '@/state'

export function DaemonHealthDashboard() {
  const items = useEvolutionEventSelector((s) => s.daemon_health)
  if (items.length === 0) {
    return (
      <p className="text-zinc-400" data-testid="daemon-empty">
        No daemon health events received yet.
      </p>
    )
  }
  return (
    <table className="w-full table-fixed border-collapse text-left" data-testid="daemon-table">
      <thead>
        <tr className="text-[10px] uppercase tracking-wide text-zinc-500">
          <th className="w-2/5 py-1">Check</th>
          <th className="w-1/5 py-1">State</th>
          <th className="w-2/5 py-1">Recovery</th>
        </tr>
      </thead>
      <tbody>
        {items.slice(-25).map((p, i) => (
          <tr key={`${p.checkName}-${i}`} className="border-t border-zinc-100">
            <td className="py-1 font-mono text-[11px]">{p.checkName}</td>
            <td className="py-1">
              <span
                className={
                  p.state === 'healthy'
                    ? 'rounded bg-emerald-100 px-1.5 py-0.5 text-emerald-700'
                    : p.state === 'degraded'
                      ? 'rounded bg-amber-100 px-1.5 py-0.5 text-amber-700'
                      : 'rounded bg-rose-100 px-1.5 py-0.5 text-rose-700'
                }
              >
                {p.state}
              </span>
            </td>
            <td className="py-1 text-[11px] text-zinc-600">
              {p.recoveryAction
                ? `${p.recoveryAction}${p.recoveryOutcome ? ' → ' + p.recoveryOutcome : ''}`
                : '—'}
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  )
}
