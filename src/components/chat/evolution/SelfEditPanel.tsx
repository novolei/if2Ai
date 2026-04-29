// UI-005 — Self-edit proposal + verification verdict 2-tab panel.

import { useState } from 'react'

import { useEvolutionEventSelector } from '@/state'

type SelfEditTab = 'proposals' | 'verdicts'

export function SelfEditPanel() {
  const proposals = useEvolutionEventSelector((s) => s.self_edit_proposal)
  const verdicts = useEvolutionEventSelector((s) => s.verification_decision)
  const [tab, setTab] = useState<SelfEditTab>('proposals')

  return (
    <div data-testid="self-edit-panel">
      <div className="mb-2 flex gap-1 text-[11px]">
        <button
          type="button"
          data-testid="self-edit-tab-proposals"
          data-active={tab === 'proposals'}
          onClick={() => setTab('proposals')}
          className={
            tab === 'proposals'
              ? 'rounded bg-zinc-900 px-2 py-0.5 text-white'
              : 'rounded bg-zinc-100 px-2 py-0.5 text-zinc-700'
          }
        >
          Proposals ({proposals.length})
        </button>
        <button
          type="button"
          data-testid="self-edit-tab-verdicts"
          data-active={tab === 'verdicts'}
          onClick={() => setTab('verdicts')}
          className={
            tab === 'verdicts'
              ? 'rounded bg-zinc-900 px-2 py-0.5 text-white'
              : 'rounded bg-zinc-100 px-2 py-0.5 text-zinc-700'
          }
        >
          Verdicts ({verdicts.length})
        </button>
      </div>
      {tab === 'proposals' ? (
        proposals.length === 0 ? (
          <p className="text-zinc-400" data-testid="self-edit-proposals-empty">
            No self-edit proposals yet.
          </p>
        ) : (
          <ul className="space-y-1.5" data-testid="self-edit-proposals-list">
            {proposals.slice(-15).map((p, i) => (
              <li key={`${p.id}-${i}`} className="rounded border border-zinc-100 p-2">
                <div className="flex items-baseline justify-between">
                  <span className="font-mono text-[11px] text-zinc-900">{p.target}</span>
                  <span className="text-[10px] text-zinc-500">{p.kind}</span>
                </div>
                <p className="mt-0.5 text-[11px] text-zinc-600">{p.justification}</p>
              </li>
            ))}
          </ul>
        )
      ) : verdicts.length === 0 ? (
        <p className="text-zinc-400" data-testid="self-edit-verdicts-empty">
          No verification verdicts yet.
        </p>
      ) : (
        <ul className="space-y-1" data-testid="self-edit-verdicts-list">
          {verdicts.slice(-15).map((v, i) => (
            <li
              key={`${v.proposalId}-${i}`}
              className="flex items-baseline justify-between rounded border border-zinc-100 p-2 text-[11px]"
            >
              <span className="font-mono text-zinc-900">{v.proposalId}</span>
              <span
                className={
                  v.verdict === 'pass'
                    ? 'rounded bg-emerald-100 px-1.5 py-0.5 text-emerald-700'
                    : 'rounded bg-rose-100 px-1.5 py-0.5 text-rose-700'
                }
              >
                {v.verdict}
                {v.failedGates.length > 0 ? ` (${v.failedGates.join(', ')})` : ''}
              </span>
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}
