// UI-006 — Browser/DK/Checkpoint/ContentSimplified composite panel.
// 4 mini-sections; each reads its own evolution slice.

import { useEvolutionEventSelector } from '@/state'

export function EvolutionMiscPanel() {
  const browser = useEvolutionEventSelector((s) => s.browser_health)
  const dk = useEvolutionEventSelector((s) => s.domain_knowledge)
  const checkpoints = useEvolutionEventSelector((s) => s.checkpoint_updated)
  const simplified = useEvolutionEventSelector((s) => s.content_simplified)

  return (
    <div className="space-y-3 text-[11px]" data-testid="misc-panel">
      <section data-testid="misc-browser">
        <h4 className="text-[10px] uppercase tracking-wide text-zinc-500">Browser Health</h4>
        {browser.length === 0 ? (
          <p className="text-zinc-400">No browser events yet.</p>
        ) : (
          <ul className="space-y-0.5">
            {browser.slice(-5).map((p, i) => (
              <li key={i} className="font-mono text-zinc-700">
                {p.sessionId} · {p.status}
                {p.recoveryPlan ? ` → ${p.recoveryPlan}` : ''}
              </li>
            ))}
          </ul>
        )}
      </section>
      <section data-testid="misc-dk">
        <h4 className="text-[10px] uppercase tracking-wide text-zinc-500">Domain Knowledge</h4>
        {dk.length === 0 ? (
          <p className="text-zinc-400">No DK events yet.</p>
        ) : (
          <ul className="space-y-0.5">
            {dk.slice(-5).map((p, i) => (
              <li key={i} className="font-mono text-zinc-700">
                {p.kind} · {p.source}
                {p.accessCount !== undefined ? ` · ×${p.accessCount}` : ''}
              </li>
            ))}
          </ul>
        )}
      </section>
      <section data-testid="misc-checkpoint">
        <h4 className="text-[10px] uppercase tracking-wide text-zinc-500">Working Checkpoint</h4>
        {checkpoints.length === 0 ? (
          <p className="text-zinc-400">No checkpoint events yet.</p>
        ) : (
          <ul className="space-y-0.5">
            {checkpoints.slice(-5).map((p, i) => (
              <li key={i} className="font-mono text-zinc-700">
                {p.sessionId} · {p.action}
                {p.keyInfoTokens !== undefined ? ` · ${p.keyInfoTokens}t` : ''}
              </li>
            ))}
          </ul>
        )}
      </section>
      <section data-testid="misc-simplified">
        <h4 className="text-[10px] uppercase tracking-wide text-zinc-500">HTML Simplified</h4>
        {simplified.length === 0 ? (
          <p className="text-zinc-400">No content simplifications yet.</p>
        ) : (
          <ul className="space-y-0.5">
            {simplified.slice(-5).map((p, i) => (
              <li key={i} className="font-mono text-zinc-700">
                {p.originalChars.toLocaleString()} → {p.simplifiedChars.toLocaleString()} chars (×
                {p.compressionRatio.toFixed(2)})
              </li>
            ))}
          </ul>
        )}
      </section>
    </div>
  )
}
