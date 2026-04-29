// UI-003 — Token-compression event history table.

import { useEvolutionEventSelector } from '@/state'

export function CompressionHistoryTable() {
  const items = useEvolutionEventSelector((s) => s.compression_event)
  if (items.length === 0) {
    return (
      <p className="text-zinc-400" data-testid="compression-empty">
        No compression events received yet.
      </p>
    )
  }
  return (
    <table
      className="w-full table-fixed border-collapse text-left"
      data-testid="compression-table"
    >
      <thead>
        <tr className="text-[10px] uppercase tracking-wide text-zinc-500">
          <th className="w-1/5 py-1">Tier</th>
          <th className="w-1/5 py-1">Kept tokens</th>
          <th className="w-1/5 py-1">Dropped</th>
          <th className="w-2/5 py-1">Mode</th>
        </tr>
      </thead>
      <tbody>
        {items.slice(-30).map((p, i) => (
          <tr key={i} className="border-t border-zinc-100">
            <td className="py-1 font-mono text-[11px]">{p.tier ?? 'recent_messages'}</td>
            <td className="py-1 text-[11px]">{p.keptTokens.toLocaleString()}</td>
            <td className="py-1 text-[11px]">{p.dropped.toLocaleString()}</td>
            <td className="py-1 text-[11px]">
              {p.passthrough ? (
                <span className="rounded bg-zinc-100 px-1.5 py-0.5 text-zinc-600">passthrough</span>
              ) : (
                <span className="rounded bg-sky-100 px-1.5 py-0.5 text-sky-700">compressed</span>
              )}
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  )
}
