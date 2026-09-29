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

/** 重采样算法（规范 6.8，默认 Lanczos3） */
export type Resample = "lanczos3" | "bicubic" | "bilinear" | "nearest";
/** 输出格式（规范 6.8） */
export type OutputFormat = "keep" | "png" | "jpeg" | "webp";
/** 未声明尺寸的 SVG 取基准的方式（规范 10.5） */
export type SvgSizeMode = "pixel" | "dpi";

export interface ImageOptions {
  resample: Resample;
  /** 质量参数，仅对 JPEG 生效；WebP 目前为无损编码（规范 6.8 / 5.4） */
  quality: number;
  format: OutputFormat;
  /** true = 保留全部元数据（含缩略图等冗余数据） */
  keepAllMetadata: boolean;
  backgroundFill: string;
}

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

/** 视频附加信息（由 ffprobe 探测，规范 12.4） */
export interface VideoInfo {
  width: number;
  height: number;
  durationSec: number;
  fps: number;
  /** 显示矩阵旋转角度 */
  rotation: number;
  isHdr: boolean;
  hasAudio: boolean;
  hasSubtitle: boolean;
  chapterCount: number;
  pixFmt: string | null;
}

/** 动图附加信息（规范 6.10） */
export interface AnimationInfo {
  width: number;
  height: number;
  /** 帧数；null 表示无法确定 */
  frames: number | null;
  /** 循环次数；0 表示无限循环 */
  loopCount: number;
  hasAlpha: boolean;
}

/** 动图处理参数（规范 6.10） */
export interface AnimationOptions {
  /** 调色板颜色数：256 / 128 / 64 */
  colors: number;
  dither: boolean;
  /** none | mp4 | webm —— 「GIF 转 MP4 / WebM」 */
  toVideo: "none" | "mp4" | "webm";
  /** 目标体积（字节）；给定时按三档策略逼近 */
  targetBytes?: number | null;
}

/** 视频编码器（规范 6.9：四种全部提供） */
export type VideoCodec = "h264" | "h265" | "vp9" | "av1";
/** 硬件加速实现（规范 6.9） */
export type VideoAccel = "nvenc" | "qsv" | "amf";

export interface VideoOptions {
  codec: VideoCodec;
  crf: number;
  preset: string;
  hardware: boolean;
  accel: VideoAccel;
  /** 默认保持 HDR 原样传递 */
  tonemapToSdr: boolean;
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
  /** 仅对 SVG 有意义：根元素是否声明了 width / height（规范 10.5） */
  svgDeclared: boolean;
  /** 视频附加信息，仅视频文件有 */
  video?: VideoInfo;
  /** 动图附加信息，仅动图有 */
  animation?: AnimationInfo;
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
  /** 动图循环次数，执行时原样写回 */
  loopCount?: number;
  ext?: string;
}

export interface ExecOptions {
  outputDir: string | null;
  keepStructure: boolean;
  onConflict: "skip" | "overwrite" | "rename";
  image: ImageOptions;
  video: VideoOptions;
  animation: AnimationOptions;
  /** 干跑：只计算与预览，不写出文件（规范 6.5） */
  dryRun: boolean;
  /** 处理前备份源文件（规范 6.5） */
  backup: boolean;
  /** 覆盖源文件（规范 6.5，默认关闭） */
  overwriteSource: boolean;
  /** 写出后校验（规范 6.7） */
  verifyOutput: boolean;
  /** 增量处理：跳过未变化的素材（规范 6.13） */
  skipUnchanged: boolean;
  /** 配置目录，供后端写日志与增量索引 */
  configDir?: string | null;
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
  /** 运行级说明（如硬件编码回退），供界面写入日志 */
  notes: string[];
  /** 本次是否为干跑（未写出任何文件） */
  dryRun: boolean;
}

/** 执行进度事件（规范 6.7） */
export interface ProgressEvent {
  done: number;
  total: number;
  current: string;
}

/** 输出目录方式（规范 6.5） */
export type OutputMode = "sibling" | "user";

/** 预设（规范 6.12）。id 内置为 builtin-*，用户预设为 user-* */
export interface Preset {
  id: string;
  name: string;
  builtin: boolean;
  /** 内置预设可隐藏但不可删除 */
  hidden: boolean;
  setting: Setting;
}

export interface PresetStore {
  version: number;
  presets: Preset[];
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

/** 界面外观（规范 6.14 / 第七节） */
export type ThemeMode = "system" | "light" | "dark";
export type Density = "compact" | "standard" | "relaxed";

/** 主工作区的一列（与 Rust 侧 LayoutColumnConfig 对应） */
export interface LayoutColumnConfig {
  id: string;
  /** files | settings | preview | run */
  panels: string[];
  width: number | null;
  tailHeight: number;
}

/** 分区布局（规范第七节的个性化布局） */
export interface LayoutConfig {
  columns: LayoutColumnConfig[];
  logHeight: number;
}

export interface ThemeSettings {
  /** 默认跟随系统外观 */
  mode: ThemeMode;
  /** 背景图绝对路径或 null（本地导入，不上传） */
  backgroundImage: string | null;
  /** 背景图透明度 0–100 */
  backgroundOpacity: number;
  /** 对比度不足时自动加蒙层 */
  autoScrim: boolean;
  /** 主题色 #RRGGBB */
  accent: string;
  density: Density;
  /** 参数设置区宽度（px） */
  sidebarWidth: number;
  /** 预览与执行列宽度（px） */
  previewWidth: number;
  /** 右列中「执行与进度区」的高度（px） */
  runHeight: number;
  /** 日志区展开时的高度（px） */
  logHeight: number;
  /** 分区布局；为 null 时按上面几个尺寸推出默认排布 */
  layout: LayoutConfig | null;
}

export const THEME_MODE_LABEL: Record<ThemeMode, string> = {
  system: "跟随系统",
  light: "浅色",
  dark: "深色",
};

export const DENSITY_LABEL: Record<Density, string> = {
  compact: "紧凑",
  standard: "标准",
  relaxed: "宽松",
};

/** 预设配色（规范第七节允许个性化配色）；天蓝为默认 */
export const ACCENT_PRESETS: { name: string; value: string }[] = [
  { name: "天蓝（默认）", value: "#2F8BD0" },
  { name: "靛蓝", value: "#2F5FA8" },
  { name: "墨绿", value: "#2F6F4F" },
  { name: "莓紫", value: "#7A4A9E" },
  { name: "砖红", value: "#B3452F" },
  { name: "奶油棕", value: "#A8763A" },
  { name: "石墨", value: "#4A4E57" },
];

/** 布局的出厂值，供「恢复默认布局」使用 */
export const DEFAULT_LAYOUT = {
  sidebarWidth: 350,
  previewWidth: 350,
  runHeight: 280,
  logHeight: 190,
} as const;

export interface Settings {
  version: number;
  language: string;
  theme: ThemeSettings;
  output: {
    directory: string;
    overwriteSource: boolean;
    keepStructure: boolean;
    onConflict: "skip" | "overwrite" | "rename";
    backgroundFillColor: string;
    /** true = 剥离缩略图等冗余元数据（规范 11.1 默认值） */
    stripRedundantMetadata: boolean;
    outputFormat: OutputFormat;
    /** 图片目标体积上限（KB）；null 表示不限制（规范 6.11） */
    targetBytesKb?: number | null;
  };
  processing: {
    concurrency: number;
    resample: Resample;
    jpgQuality: number;
    videoCrf: number;
    videoEncoder: string;
    hardwareAccel: boolean;
    videoAccel: VideoAccel;
    hdrTonemapToSdr: boolean;
    gifColors: number;
    gifDither: boolean;
    upscaleWarnThreshold: number;
    svgDpi: number;
    svgSizeMode: SvgSizeMode;
  };
  grouping: Grouping;
  recentFolders: string[];
}

export const RESAMPLE_LABEL: Record<Resample, string> = {
  lanczos3: "Lanczos3（默认，平滑）",
  bicubic: "Bicubic",
  bilinear: "Bilinear",
  nearest: "Nearest（像素风）",
};

export const FORMAT_LABEL: Record<OutputFormat, string> = {
  keep: "保持原格式",
  png: "统一转为 PNG",
  jpeg: "统一转为 JPEG",
  webp: "统一转为 WebP（无损）",
};

export const CODEC_LABEL: Record<VideoCodec, string> = {
  h264: "H.264（兼容性最好）",
  h265: "H.265 / HEVC（同画质更省体积）",
  vp9: "VP9",
  av1: "AV1（压缩率最高，编码较慢）",
};

export const GIF_COLORS: number[] = [256, 128, 64];

export const ACCEL_LABEL: Record<VideoAccel, string> = {
  nvenc: "NVIDIA NVENC",
  qsv: "Intel QSV",
  amf: "AMD AMF",
};

/** FFmpeg 自检结果（规范 12.5） */
export interface FfmpegCheck {
  ffmpegFound: boolean;
  missingEncoders: string[];
  missingFilters: string[];
  missingHevcDecoder: boolean;
  summary: string;
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
