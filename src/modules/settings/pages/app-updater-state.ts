import type { UpdaterCheckResult, UpdaterRuntimeState } from "@/api/updater";

export type AppUpdaterUiState =
  | "idle"
  | "checking"
  | "available"
  | "downloading"
  | "downloaded"
  | "installing"
  | "ready"
  | "failed";

export const APP_UPDATER_STATE_LABELS: Record<
  AppUpdaterUiState,
  { title: string; description: string }
> = {
  idle: {
    title: "尚未检查更新",
    description: "可以手动检查更新；开启自动检查后，If2Ai 会低频确认新版本。",
  },
  checking: {
    title: "正在检查",
    description: "正在连接更新服务并确认可用版本。",
  },
  available: {
    title: "发现新版本",
    description: "更新包已通过签名通道确认，可以下载安装。",
  },
  downloading: {
    title: "正在下载",
    description: "正在下载并校验更新包；完成后会准备安装。",
  },
  downloaded: {
    title: "更新包已下载",
    description: "更新包已完成下载和签名校验，正在准备安装。",
  },
  installing: {
    title: "正在安装",
    description: "系统安装器已接管更新流程，If2Ai 可能会自动退出或重启。",
  },
  ready: {
    title: "已是最新",
    description: "当前安装版本已经是可用通道中的最新版本。",
  },
  failed: {
    title: "无法检查更新",
    description: "无法连接更新服务。请检查网络，或稍后重试。",
  },
};

export function updaterUiStateFromResult(
  result: UpdaterCheckResult | null,
): AppUpdaterUiState {
  if (!result) return "idle";
  if (result.status === "update_available") return "available";
  if (result.status === "no_update") return "ready";
  return "failed";
}

export function updaterUiStateFromRuntime(
  state: UpdaterRuntimeState | null,
): AppUpdaterUiState {
  if (!state) return "idle";
  if (state.status === "latest") return "ready";
  if (state.status === "error") return "failed";
  return state.status;
}

export function updaterProgressPercent(
  downloaded?: number | null,
  total?: number | null,
): number | null {
  if (!downloaded || !total || total <= 0) return null;
  return Math.max(0, Math.min(100, Math.round((downloaded / total) * 100)));
}
