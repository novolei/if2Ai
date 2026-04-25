import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getApiClient } from "./client.ts";

export type UpdaterRuntimeStatus =
  | "idle"
  | "checking"
  | "available"
  | "downloading"
  | "downloaded"
  | "installing"
  | "latest"
  | "error";

export type UpdaterCheckStatus =
  | "no_update"
  | "update_available"
  | "failed";

export interface UpdaterRuntimeState {
  status: UpdaterRuntimeStatus;
  current_version: string;
  manifest_url?: string | null;
  channel: "stable" | "beta" | "nightly";
  auto_check_enabled: boolean;
  latest_version?: string | null;
  release_notes_url?: string | null;
  artifact_url?: string | null;
  downloaded_bytes?: number | null;
  total_bytes?: number | null;
  checked_at?: string | null;
  diagnostic?: string | null;
}

export interface UpdaterCheckResult {
  status: UpdaterCheckStatus;
  current_version: string;
  latest_version?: string | null;
  manifest_url?: string | null;
  artifact_url?: string | null;
  artifact_checksum_sha256?: string | null;
  release_notes_url?: string | null;
  diagnostic?: string | null;
}

export type UpdaterDownloadStatus =
  | "downloaded"
  | "installing"
  | "no_update"
  | "failed";

export interface UpdaterDownloadResult {
  status: UpdaterDownloadStatus;
  current_version: string;
  latest_version?: string | null;
  artifact_url?: string | null;
  local_path?: string | null;
  checksum_sha256?: string | null;
  diagnostic?: string | null;
}

export interface UpdaterPreferences {
  auto_check_enabled: boolean;
  channel: "stable" | "beta" | "nightly";
}

/** Load the idle updater state and configured manifest URL. */
export async function getAppUpdaterState(): Promise<UpdaterRuntimeState> {
  return getApiClient().call<UpdaterRuntimeState>("app_updater_get_state");
}

/** Check the signed Tauri updater endpoint. */
export async function checkAppUpdater(): Promise<UpdaterCheckResult> {
  return getApiClient().call<UpdaterCheckResult>("app_updater_check");
}

/** Check the legacy release manifest, optionally overriding its URL. */
export async function checkAppUpdaterManifest(
  manifestUrl?: string | null,
): Promise<UpdaterCheckResult> {
  return getApiClient().call<UpdaterCheckResult>("app_updater_check_manifest", {
    manifestUrl: manifestUrl ?? null,
  });
}

/** Download, verify, and install the signed update bundle. */
export async function downloadAndInstallAppUpdate(): Promise<UpdaterDownloadResult> {
  return getApiClient().call<UpdaterDownloadResult>(
    "app_updater_download_and_install",
  );
}

/** Legacy path: download the artifact, verify it, and open it with the OS handler. */
export async function downloadAndOpenAppUpdate(
  manifestUrl?: string | null,
): Promise<UpdaterDownloadResult> {
  return getApiClient().call<UpdaterDownloadResult>(
    "app_updater_download_and_open",
    {
      manifestUrl: manifestUrl ?? null,
    },
  );
}

/** Persist updater preferences. */
export async function setAppUpdaterPreferences(
  preferences: UpdaterPreferences,
): Promise<UpdaterRuntimeState> {
  return getApiClient().call<UpdaterRuntimeState>(
    "app_updater_set_preferences",
    { preferences },
  );
}

/** Subscribe to updater runtime state events emitted by the backend. */
export async function onAppUpdaterState(
  handler: (state: UpdaterRuntimeState) => void,
): Promise<UnlistenFn> {
  return listen<UpdaterRuntimeState>("app-updater://state", (event) => {
    handler(event.payload);
  });
}
