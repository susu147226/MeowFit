import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

import type {
  AppInfo,
  ExecOptions,
  ExecReport,
  FfmpegCheck,
  Group,
  Grouping,
  Plan,
  PlanRequest,
  PresetConfig,
  PresetStore,
  Setting,
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

  /** 读取本地背景图并转成 data URL（仅用于界面外观，不上传） */
  readBackgroundImage: (path: string) =>
    invoke<string>("read_background_image", { path }),

  /** FFmpeg 可用性与构建自检（规范 12.5） */
  ffmpegSelfCheck: () => invoke<FfmpegCheck>("ffmpeg_self_check"),

  /** 取消当前执行（规范 6.7） */
  cancelExecution: () => invoke<void>("cancel_execute"),

  /** 目标磁盘剩余空间（规范 6.6） */
  diskFreeSpace: (path: string) => invoke<number>("disk_free_space", { path }),

  /** 预设（规范 6.12） */
  loadPresets: () => invoke<PresetStore>("load_presets"),
  addPreset: (name: string, setting: Setting, config: PresetConfig | null) =>
    invoke<PresetStore>("add_preset", { name, setting, config }),
  renamePreset: (id: string, name: string) =>
    invoke<PresetStore>("rename_preset", { id, name }),
  removePreset: (id: string) => invoke<PresetStore>("remove_preset", { id }),
  exportPresets: () => invoke<string>("export_presets"),
  importPresets: (json: string) => invoke<PresetStore>("import_presets", { json }),
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

/** 选择界面背景图（规范 6.14，仅本地导入，不上传）。 */
export async function pickBackgroundImage(): Promise<string | null> {
  const picked = await open({
    multiple: false,
    title: "选择界面背景图",
    filters: [{ name: "图片", extensions: ["png", "jpg", "jpeg", "webp", "gif", "bmp", "avif"] }],
  });
  return typeof picked === "string" ? picked : null;
}
