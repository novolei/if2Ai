// MIG-012 — onboarding domain facade.

import { getApiClient } from './client.ts'

/** Fetch the current onboarding state bag. Shape is opaque to
 * this layer — callers own the interpretation. */
export async function getOnboardingState(): Promise<Record<string, unknown>> {
  return getApiClient().call<Record<string, unknown>>('onboarding_get_state')
}
