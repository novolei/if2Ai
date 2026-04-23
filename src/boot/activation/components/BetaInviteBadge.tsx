/**
 * Top-right "内测邀请码" capsule badge with the orange→red gradient.
 *
 * UClaw uses SF Symbol `sparkles`; we substitute lucide-react's
 * `Sparkles` for visual parity.
 */

import { Sparkles } from 'lucide-react'

export function BetaInviteBadge({ label = '内测邀请码' }: { label?: string }) {
  return (
    <div
      className="inline-flex items-center gap-1.5 rounded-full px-3 py-[7px] text-[12px] font-semibold text-white"
      style={{
        background:
          'linear-gradient(to right, rgba(255,135,51,0.96), rgba(242,77,26,0.96))',
        boxShadow: '0 5px 10px rgba(243,89,31,0.32)',
        outline: '1px solid rgba(255,255,255,0.34)',
      }}
    >
      <Sparkles className="h-3.5 w-3.5" />
      <span>{label}</span>
    </div>
  )
}
