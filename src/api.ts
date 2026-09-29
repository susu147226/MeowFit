import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

import type {
  AppInfo,
  ExecOptions,
  ExecReport,
  Group,
  Grouping,
  Plan,
  PlanRequest,
  ScanOptions,
  ScanResult,
  Settings,
  SourceRef,
} from "./types";

/** 所有 Tauri 命令的唯一出口，参数名与 Rust 侧签名一一对应。 */
export const api = {
  appInfo: () => invoke<AppInfo>("get_app_info"),

  scanFolder: (root: string, options: ScanOptions) =>
    invoke<ScanResult>("scan_folder", { root, options }),

  groupEntries: (
    files: { id: string; name: string; relativeParent: string }[],
    grouping: Grouping,
  ) => invoke<Group[]>("group_entries", { files, grouping }),

  previewPlan: (request: PlanRequest) => invoke<Plan>("preview_plan", { request }),

  executePlan: (
    request: PlanRequest,
    sources: SourceRef[],
    options: ExecOptions,
    root: string,
  ) => invoke<ExecReport>("execute_plan", { request, sources, options, root }),

  loadSettings: () => invoke<Settings>("load_settings"),

  saveSettings: (settings: Settings) => invoke<Settings>("save_settings", { settings }),

  touchRecentFolder: (folder: string) => invoke<Settings>("touch_recent_folder", { folder }),
};

/** 选择素材文件夹（本地路径，不经任何网络）。 */
export async function pickFolder(): Promise<string | null> {
  const picked = await open({ directory: true, multiple: false, title: "选择素材文件夹" });
  return typeof picked === "string" ? picked : null;
}

/** 选择输出目录（规范 6.5 的「每次任务由用户指定」方式）。 */
export async function pickOutputFolder(): Promise<string | null> {
  const picked = await open({ directory: true, multiple: false, title: "选择输出目录" });
  return typeof picked === "string" ? picked : null;
}
