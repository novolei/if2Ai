// UI-004 — Skill sedimentation timeline.

import { useEvolutionEventSelector } from '@/state'

export function SkillSedimentationTimeline() {
  const items = useEvolutionEventSelector((s) => s.skill_sedimented)
  if (items.length === 0) {
    return (
      <p className="text-zinc-400" data-testid="skills-empty">
        No skills sedimented yet. (LLM is currently a placeholder; production wire-up pending.)
      </p>
    )
  }
  return (
    <ol className="space-y-2" data-testid="skills-timeline">
      {items.slice(-20).map((p, i) => (
        <li key={`${p.name}-${i}`} className="rounded border border-zinc-100 p-2">
          <div className="flex items-baseline justify-between gap-2">
            <span className="font-mono text-[11px] font-medium text-zinc-900">{p.name}</span>
            <span className="text-[10px] text-zinc-400">turns: [{p.sourceTurns.join(', ')}]</span>
          </div>
          <p className="mt-1 text-[11px] text-zinc-600">{p.description}</p>
          <div className="mt-1 flex flex-wrap gap-1">
            {p.toolSequence.map((tool, ti) => (
              <span
                key={`${tool}-${ti}`}
                className="rounded bg-violet-100 px-1.5 py-0.5 text-[10px] font-mono text-violet-700"
              >
                {tool}
              </span>
            ))}
          </div>
        </li>
      ))}
    </ol>
  )
}
