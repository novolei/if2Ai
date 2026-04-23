/**
 * Activation gate backdrop — frosted glass + radial glows.
 *
 * 1:1 visual port of UClaw's `ActivationGateBackgroundView.swift`:
 * full-screen `backdrop-blur` over the host shell, two large radial
 * highlights (lower-left + upper-right), a faint top-to-bottom dark
 * vignette to keep the white from feeling washed-out.
 */

export function GateBackdrop() {
  return (
    <div className="pointer-events-none absolute inset-0 overflow-hidden">
      <div className="absolute inset-0 bg-white/40 backdrop-blur-2xl" />
      <div
        className="absolute inset-0 opacity-95"
        style={{
          background:
            'linear-gradient(135deg, rgba(255,255,255,0.10), rgba(238,242,247,0.14), rgba(255,255,255,0.08))',
        }}
      />
      <div
        className="absolute"
        style={{
          width: 560,
          height: 560,
          left: 'calc(50% - 560px)',
          top: 'calc(50% + 80px)',
          filter: 'blur(10px)',
          background:
            'radial-gradient(circle, rgba(255,255,255,0.18) 0%, rgba(255,255,255,0.08) 40%, rgba(255,255,255,0) 70%)',
        }}
      />
      <div
        className="absolute"
        style={{
          width: 460,
          height: 460,
          right: 'calc(50% - 460px - 60px)',
          top: 'calc(50% - 460px + 60px)',
          filter: 'blur(12px)',
          background:
            'radial-gradient(circle, rgba(255,255,255,0.14) 0%, rgba(240,243,247,0.07) 40%, rgba(255,255,255,0) 70%)',
        }}
      />
      <div
        className="absolute inset-0"
        style={{
          background:
            'linear-gradient(to bottom, rgba(0,0,0,0) 0%, rgba(0,0,0,0.025) 100%)',
        }}
      />
    </div>
  )
}
