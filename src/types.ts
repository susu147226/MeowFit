// 与 Rust 侧 serde 输出一一对应（src-tauri/src/model.rs、scan.rs、config.rs、exec.rs）

export type Mode = "A" | "B" | "C" | "D" | "E" | "G";
export type Anchor =
  | "topLeft"
  | "top"
  | "topRight"
  | "left"
  | "center"
  | "right"
  | "bottomLeft"
  | "bottom"
  | "bottomRight";
export type Grouping = "prefix" | "extension" | "folder";
export type MediaKind = "raster" | "svg" | "animated" | "video";
export type Action = "unchanged" | "resize";
export type Status = "success" | "unchanged" | "skipped" | "failed";
export type SettingSource = "file" | "group" | "global" | "none";
export type ConfigMode = "portable" | "installed" | "temporary";

/** 一层缩放参数（规范 6.2 / 6.3） */
export interface Setting {
  mode: Mode;
  scale?: number;
  width?: number;
  height?: number;
  noPad?: boolean;
  limit?: number;
  onlyDown?: boolean;
  onlyUp?: boolean;
  anchor?: Anchor;
}

/** 三态：null = 未设置，followGlobal = 显式跟随整体，否则为具体设置（规范 10.2） */
export type GroupTier = { kind: "followGlobal" } | { kind: "explicit"; setting: Setting };
export type GroupTierState = GroupTier | null;

export interface Computed {
  width: number;
  height: number;
  contentWidth: number;
  contentHeight: number;
  anchor: Anchor;
}

export interface AppError {
  code: string;
  message: string;
}

export interface PlanEntry {
  id: string;
  name: string;
  group: string;
  source: SettingSource;
  action: Action;
  originalWidth: number;
  originalHeight: number;
  target?: Computed;
  mode?: Mode;
  error?: AppError;
}

export interface Plan {
  entries: PlanEntry[];
  ok: boolean;
}

export interface ScannedFile {
  id: string;
  path: string;
  relativePath: string;
  relativeParent: string;
  name: string;
  ext: string;
  kind: MediaKind | null;
  size: number;
  mtimeMs: number;
  width: number | null;
  height: number | null;
  skipReason: string | null;
}

export interface ScanResult {
  root: string;
  outputDir: string;
  files: ScannedFile[];
}

export interface ScanOptions {
  recursive: boolean;
  include: string[];
  exclude: string[];
}

export interface Group {
  name: string;
  fileIds: string[];
}

export interface PlanFileInput {
  id: string;
  name: string;
  width: number;
  height: number;
  isVideo: boolean;
  group: string;
  setting: Setting | null;
}

export interface PlanGroupInput {
  name: string;
  setting: GroupTierState;
}

export interface PlanRequest {
  files: PlanFileInput[];
  groups: PlanGroupInput[];
  global: Setting | null;
}

export interface SourceRef {
  id: string;
  path: string;
  kind: MediaKind;
}

export interface ExecOptions {
  outputDir: string | null;
  keepStructure: boolean;
  onConflict: "skip" | "overwrite" | "rename";
  backgroundFillColor: string;
}

export interface FileOutcome {
  id: string;
  status: Status;
  outputPath?: string;
  originalSize: number;
  newSize?: number;
  reason?: string;
}

export interface OutcomeCounts {
  total: number;
  success: number;
  unchanged: number;
  skipped: number;
  failed: number;
}

export interface ExecReport {
  outputDir: string;
  outcomes: FileOutcome[];
  counts: OutcomeCounts;
}

export interface AppInfo {
  name: string;
  version: string;
  identifier: string;
  copyright: string;
  licenseName: string;
  configDir: string;
  configMode: ConfigMode;
  configPersistent: boolean;
}

export interface Settings {
  version: number;
  language: string;
  theme: { mode: "light" | "dark"; backgroundImage: string | null; backgroundOpacity: number };
  output: {
    directory: string;
    overwriteSource: boolean;
    keepStructure: boolean;
    onConflict: "skip" | "overwrite" | "rename";
    backgroundFillColor: string;
    stripRedundantMetadata: boolean;
  };
  processing: Record<string, unknown>;
  grouping: Grouping;
  recentFolders: string[];
}

export const MODE_LABEL: Record<Mode, string> = {
  A: "A 等比缩放（居中）",
  B: "B 自定义宽高（等比适配 fit）",
  C: "C 自定义宽高（填满裁剪 cover）",
  D: "D 自定义宽高（拉伸 stretch）",
  E: "E 限制最大边",
  G: "G 倍数锚点缩放",
};

export const MODE_SHORT: Record<Mode, string> = {
  A: "A 等比",
  B: "B 适配",
  C: "C 裁剪",
  D: "D 拉伸",
  E: "E 限长边",
  G: "G 锚点",
};

export const SOURCE_LABEL: Record<SettingSource, string> = {
  file: "单文件",
  group: "分组",
  global: "整体",
  none: "不变",
};

export const STATUS_LABEL: Record<Status, string> = {
  success: "成功",
  unchanged: "未改动",
  skipped: "已跳过",
  failed: "失败",
};

export const GROUPING_LABEL: Record<Grouping, string> = {
  prefix: "按命名前缀",
  extension: "按扩展名",
  folder: "按所在子文件夹",
};

export const ANCHOR_LABEL: Record<Anchor, string> = {
  topLeft: "左上",
  top: "上",
  topRight: "右上",
  left: "左",
  center: "居中",
  right: "右",
  bottomLeft: "左下",
  bottom: "下",
  bottomRight: "右下",
};

export const MODES: Mode[] = ["A", "B", "C", "D", "E", "G"];

export function formatBytes(n: number | undefined | null): string {
  if (n === undefined || n === null) return "—";
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / 1024 / 1024).toFixed(2)} MB`;
}
