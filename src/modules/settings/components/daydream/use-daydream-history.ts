import { useRuntimeProjectionSelector } from '@/runtime-projection/use-runtime-projection'
import type { DaydreamReportPayload } from '@/transport/contracts'

/// Selector hook that returns the cap-20 newest-first daydream history
/// from the runtime projection snapshot.
export function useDaydreamHistory(): DaydreamReportPayload[] {
  return useRuntimeProjectionSelector((s) => s.daydreamHistory)
}
