import { getApiClient } from "./client.ts";

export type UpdaterRuntimeStatus = "idle";

export type UpdaterCheckStatus =
  | "no_update"
  | "update_available"
  | "failed";

export interface UpdaterRuntimeState {
  status: UpdaterRuntimeStatus;
  current_version: string;
  manifest_url?: string | null;
  channel: "stable" | "beta" | "nightly";
}

export interface UpdaterCheckResult {
  status: UpdaterCheckStatus;
  current_version: string;
  latest_version?: string | null;
  manifest_url?: string | null;
  artifact_url?: string | null;
  release_notes_url?: string | null;
  diagnostic?: string | null;
}

/** Load the idle updater state and configured manifest URL. */
export async function getAppUpdaterState(): Promise<UpdaterRuntimeState> {
  return getApiClient().call<UpdaterRuntimeState>("app_updater_get_state");
}

/** Check the configured release manifest, optionally overriding its URL. */
export async function checkAppUpdater(
  manifestUrl?: string | null,
): Promise<UpdaterCheckResult> {
  return getApiClient().call<UpdaterCheckResult>("app_updater_check", {
    manifestUrl: manifestUrl ?? null,
  });
}
