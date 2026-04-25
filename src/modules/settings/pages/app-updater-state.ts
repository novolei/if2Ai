import type { UpdaterCheckResult } from "@/api/updater";

export type AppUpdaterUiState =
  | "idle"
  | "checking"
  | "available"
  | "downloading"
  | "ready"
  | "failed";

export const APP_UPDATER_STATE_LABELS: Record<
  AppUpdaterUiState,
  { title: string; description: string }
> = {
  idle: {
    title: "等待检查",
    description: "手动检查 release manifest，不会在后台静默安装。",
  },
  checking: {
    title: "正在检查",
    description: "正在读取 release manifest 并校验 artifact 元数据。",
  },
  available: {
    title: "发现新版本",
    description: "manifest 已通过校验，可打开下载链接手动安装。",
  },
  downloading: {
    title: "准备下载",
    description: "下载/安装动作留给后续 Pack；当前版本只开放 artifact 链接。",
  },
  ready: {
    title: "已是最新",
    description: "当前安装版本不低于 release manifest 中的最新版本。",
  },
  failed: {
    title: "检查失败",
    description: "manifest 未配置、网络失败或 release 元数据未通过校验。",
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
