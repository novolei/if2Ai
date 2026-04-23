/**
 * Activation card stripes — white base + upper-right red/orange
 * diagonal-stripe gradient, masked to taper into the lower-left.
 *
 * Direct port of UClaw's `ActivationGateCardStripesView.swift` using
 * SVG (no Canvas) so it stays crisp on retina + dark-mode neutral.
 */

const STRIPE_SPACING = 18
const STRIPE_WIDTH = 1.7

export function GateCardStripes() {
  const stripes = []
  const horizon = 1200
  for (let i = -100; i < 100; i++) {
    const offset = i * STRIPE_SPACING
    stripes.push(
      <polygon
        key={i}
        points={`${offset - horizon},0 ${offset},${horizon} ${offset + STRIPE_WIDTH},${horizon} ${offset - horizon + STRIPE_WIDTH},0`}
        fill="rgba(255,255,255,0.24)"
      />,
    )
  }

  return (
    <div className="pointer-events-none absolute inset-0 overflow-hidden rounded-[26px]">
      {/* Base card colour */}
      <div
        className="absolute inset-0"
        style={{
          background:
            'linear-gradient(135deg, rgba(255,255,255,0.97), rgba(255,247,242,0.95) 60%, rgba(255,255,255,0.94))',
        }}
      />

      {/* Upper-right stripe gradient layer with mask */}
      <div
        className="absolute inset-0"
        style={{
          WebkitMaskImage:
            'linear-gradient(to top right, transparent 0%, transparent 28%, rgba(255,255,255,0.55) 62%, white 100%)',
          maskImage:
            'linear-gradient(to top right, transparent 0%, transparent 28%, rgba(255,255,255,0.55) 62%, white 100%)',
        }}
      >
        <div
          className="absolute inset-0"
          style={{
            background:
              'linear-gradient(to bottom right, rgb(255,89,26), rgb(255,115,36) 50%, rgb(255,148,51))',
          }}
        />
        <svg
          className="absolute inset-0 h-full w-full"
          viewBox="0 0 1200 1200"
          preserveAspectRatio="none"
          aria-hidden
        >
          {stripes}
        </svg>
      </div>

      {/* Subtle highlight + bottom-right shadow for depth */}
      <div
        className="absolute inset-0"
        style={{
          background:
            'linear-gradient(135deg, rgba(255,255,255,0.14) 0%, transparent 50%, rgba(0,0,0,0.06) 100%)',
        }}
      />
    </div>
  )
}
