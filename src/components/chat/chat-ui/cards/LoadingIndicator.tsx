/**
 * LoadingIndicator — wave-dot animation shown while the assistant
 * stream is pending.
 *
 * Extracted from `src/components/ui/chat-ui.tsx` (GF-01 PR-01) verbatim;
 * all WaveDots numerical knobs are preserved.
 */

import * as React from 'react'
import { WaveDotsAnimation } from '@/components/loading/WaveDotsAnimation'

/** Render the chat transcript loading indicator. */
export function LoadingIndicator(): React.JSX.Element {
  return (
    <div className="flex items-center px-0.5 py-1">
      <WaveDotsAnimation
        amplitude={11.04}
        ballRadius={3}
        count={6}
        delay={0.19}
        horizontalStretch={1.10625}
        topStartColor="#fb923c"
        topEndColor="#f97316"
        bottomStartColor="#f59e0b"
        bottomEndColor="#ea580c"
        className="opacity-85"
      />
    </div>
  )
}
