import { useCallback, useEffect, useState } from "react";
import { toast } from "sonner";
import {
  checkAppUpdater,
  downloadAndInstallAppUpdate,
  getAppUpdaterState,
  onAppUpdaterState,
  type UpdaterRuntimeState,
  type UpdaterRuntimeStatus,
} from "@/api/updater";

const DISMISSED_BANNER_KEY = "if2ai:app-updater:dismissed-banner-version";
const LAST_CHECK_KEY = "if2ai:app-updater:last-check-ms";
const LAST_TOAST_KEY = "if2ai:app-updater:last-toast-version";
const SIX_HOURS_MS = 6 * 60 * 60 * 1000;

export interface UseUpdaterBannerResult {
  appUpdaterState: UpdaterRuntimeState | null;
  latestUpdaterVersion: string | null;
  updaterBannerVisible: boolean;
  dismiss: () => void;
  runUpdater: () => Promise<void>;
}

/**
 * App-shell updater banner controller (GF-03 PR-1 extraction):
 *  - Subscribes to backend `UpdaterRuntimeState` (`onAppUpdaterState`).
 *  - Performs a throttled (6h) background `checkAppUpdater()` and
 *    surfaces a single sonner toast per new version.
 *  - Persists the dismissed banner version to localStorage so the user
 *    only sees it once per release.
 *  - Exposes `runUpdater()` matching the previous nav-rail handler so
 *    `<AppShell navbar={...}>` keeps the same contract.
 */
export function useUpdaterBanner(): UseUpdaterBannerResult {
  const [appUpdaterState, setAppUpdaterState] =
    useState<UpdaterRuntimeState | null>(null);
  const [dismissedBannerVersion, setDismissedBannerVersion] = useState<
    string | null
  >(() =>
    typeof window === "undefined"
      ? null
      : localStorage.getItem(DISMISSED_BANNER_KEY),
  );

  // Boot: fetch initial state + subscribe to backend state stream.
  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | null = null;
    void getAppUpdaterState()
      .then((state) => {
        if (!cancelled) setAppUpdaterState(state);
      })
      .catch((error) => {
        console.debug("[app-updater] failed to load state", error);
      });
    void onAppUpdaterState((state) => {
      setAppUpdaterState(state);
    }).then((dispose) => {
      if (cancelled) dispose();
      else unlisten = dispose;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  // Throttled background check (6h).
  useEffect(() => {
    let cancelled = false;
    const now = Date.now();
    const lastCheck = Number(localStorage.getItem(LAST_CHECK_KEY) || "0");
    if (now - lastCheck < SIX_HOURS_MS) return;
    localStorage.setItem(LAST_CHECK_KEY, String(now));
    void getAppUpdaterState()
      .then((state) => {
        if (!cancelled) setAppUpdaterState(state);
        if (!state.auto_check_enabled) return null;
        return checkAppUpdater();
      })
      .then((result) => {
        if (!result) return;
        if (!cancelled) {
          setAppUpdaterState((state) =>
            state
              ? {
                  ...state,
                  status:
                    result.status === "update_available"
                      ? "available"
                      : result.status === "no_update"
                        ? "latest"
                        : "error",
                  latest_version: result.latest_version ?? state.latest_version,
                  release_notes_url:
                    result.release_notes_url ?? state.release_notes_url,
                  artifact_url: result.artifact_url ?? state.artifact_url,
                  diagnostic: result.diagnostic ?? null,
                }
              : state,
          );
        }
        if (cancelled || result.status !== "update_available") return;
        const latest = result.latest_version ?? "新版本";
        if (localStorage.getItem(LAST_TOAST_KEY) === latest) return;
        localStorage.setItem(LAST_TOAST_KEY, latest);
        toast.info(`If2Ai ${latest} 可更新`, {
          description:
            "已发现 GitHub Release 更新包，可在设置 > 关于中下载并安装。",
          duration: 9000,
        });
      })
      .catch((error) => {
        console.debug("[app-updater] background check failed", error);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const latestUpdaterVersion = appUpdaterState?.latest_version ?? null;
  const updaterBannerVisible =
    appUpdaterState?.status === "available" &&
    Boolean(latestUpdaterVersion) &&
    dismissedBannerVersion !== latestUpdaterVersion;

  const dismiss = useCallback(() => {
    const version = latestUpdaterVersion;
    if (!version) return;
    localStorage.setItem(DISMISSED_BANNER_KEY, version);
    setDismissedBannerVersion(version);
  }, [latestUpdaterVersion]);

  const runUpdater = useCallback(async () => {
    const status = appUpdaterState?.status;
    if (
      status === "checking" ||
      status === "downloading" ||
      status === "installing"
    ) {
      return;
    }

    if (status !== "available") {
      setAppUpdaterState((state) =>
        state ? { ...state, status: "checking", diagnostic: null } : state,
      );
      const result = await checkAppUpdater();
      if (result.status !== "update_available") {
        setAppUpdaterState((state) =>
          state
            ? {
                ...state,
                status: result.status === "no_update" ? "latest" : "error",
                diagnostic: result.diagnostic ?? null,
                checked_at: new Date().toISOString(),
              }
            : state,
        );
        if (result.status === "no_update") {
          toast.success("已是最新版本");
        } else {
          toast.error("检查更新失败", {
            description: result.diagnostic ?? "请稍后重试。",
          });
        }
        return;
      }
      setAppUpdaterState((state) =>
        state
          ? {
              ...state,
              status: "available",
              latest_version: result.latest_version ?? state.latest_version,
              release_notes_url:
                result.release_notes_url ?? state.release_notes_url,
              artifact_url: result.artifact_url ?? state.artifact_url,
              diagnostic: null,
              checked_at: new Date().toISOString(),
            }
          : state,
      );
    }

    setAppUpdaterState((state) =>
      state ? { ...state, status: "downloading", diagnostic: null } : state,
    );
    try {
      const result = await downloadAndInstallAppUpdate();
      if (result.status === "installing" || result.status === "downloaded") {
        const nextStatus: UpdaterRuntimeStatus = result.status;
        setAppUpdaterState((state) =>
          state
            ? {
                ...state,
                status: nextStatus,
                latest_version: result.latest_version ?? state.latest_version,
                artifact_url: result.artifact_url ?? state.artifact_url,
                diagnostic: null,
              }
            : state,
        );
        toast.success("更新安装已启动", {
          description: "系统安装器已接管流程，If2Ai 可能会自动退出或重启。",
        });
      } else if (result.status === "no_update") {
        setAppUpdaterState((state) =>
          state ? { ...state, status: "latest", diagnostic: null } : state,
        );
        toast.success("已是最新版本");
      } else {
        setAppUpdaterState((state) =>
          state
            ? {
                ...state,
                status: "error",
                diagnostic: result.diagnostic ?? null,
              }
            : state,
        );
        toast.error("下载更新失败", {
          description: result.diagnostic ?? "请稍后重试。",
        });
      }
    } catch (error) {
      setAppUpdaterState((state) =>
        state
          ? { ...state, status: "error", diagnostic: String(error) }
          : state,
      );
      toast.error("下载更新失败", { description: String(error) });
    }
  }, [appUpdaterState?.status]);

  return {
    appUpdaterState,
    latestUpdaterVersion,
    updaterBannerVisible,
    dismiss,
    runUpdater,
  };
}
