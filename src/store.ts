import { create } from "zustand";

import { api } from "./api";
import { buildRequest, coalesce, type Basis, type Scope } from "./lib/planInput";
import { DEFAULT_LAYOUT, fromLegacy, parseLayout, type Layout } from "./lib/layout";
import { accentPalette } from "./lib/color";
import type {
  AppInfo,
  ExecReport,
  FfmpegCheck,
  Group,
  GroupTierState,
  Grouping,
  Plan,
  ScanOptions,
  ScannedFile,
  Setting,
  Settings,
  VideoCodec,
} from "./types";

export interface LogEntry {
  time: string;
  level: "INFO" | "WARN" | "ERROR";
  message: string;
}

export type { Basis, Scope };

const MAX_LOGS = 500;

interface MeowState {
  info: AppInfo | null;
  settings: Settings | null;
  /** 解析后的实际外观（供渲染使用） */
  resolvedTheme: "light" | "dark";
  /** 背景图的 data URL；背景图仅服务界面外观（规范 6.14） */
  backgroundUrl: string | null;
  /** 已加载背景图的路径，用于避免每次外观变化都重新读取文件 */
  backgroundPath: string | null;
  aboutOpen: boolean;
  appearanceOpen: boolean;
  /** 分区布局：用户拖出来的排布（规范第七节的个性化布局） */
  layout: Layout;

  root: string | null;
  scanOptions: ScanOptions;
  files: ScannedFile[];
  outputDir: string | null;
  scanning: boolean;

  grouping: Grouping;
  groups: Group[];
  /** 存在手动修正（重命名 / 移动 / 合并）时为 true */
  groupsDirty: boolean;
  /** 待切换的分组方式；非空表示正在等待确认或即将生效 */
  pendingGrouping: Grouping | null;

  globalSetting: Setting | null;
  groupTiers: Record<string, GroupTierState>;
  fileSettings: Record<string, Setting>;

  linkEnabled: boolean;
  basis: Basis;
  /** 「GIF 转 MP4 / WebM」；none 表示保持动图格式（规范 6.10） */
  animationToVideo: "none" | "mp4" | "webm";
  /** 动图目标体积（字节）；null 表示不限制 */
  animationTargetBytes: number | null;

  scope: Scope;
  selectedIds: string[];

  plan: Plan | null;
  report: ExecReport | null;
  running: boolean;
  /** FFmpeg 可用性自检结果（规范 12.5） */
  ffmpegCheck: FfmpegCheck | null;
  logs: LogEntry[];

  init: () => Promise<void>;
  log: (level: LogEntry["level"], message: string) => void;
  applyAppearance: () => Promise<void>;
  setTheme: (patch: Partial<Settings["theme"]>) => Promise<void>;
  setAboutOpen: (open: boolean) => void;
  setAppearanceOpen: (open: boolean) => void;
  applyLayout: (next: Layout) => void;

  setScanOptions: (patch: Partial<ScanOptions>) => void;
  scanFolder: (path: string, options?: Partial<ScanOptions>) => Promise<void>;

  requestGrouping: (grouping: Grouping) => void;
  confirmGrouping: () => Promise<void>;
  cancelGrouping: () => void;
  renameGroup: (from: string, to: string) => void;
  moveFile: (fileId: string, toGroup: string) => void;
  mergeGroups: (from: string, into: string) => void;

  setScope: (scope: Scope) => void;
  toggleSelect: (id: string, additive: boolean) => void;
  setLinkEnabled: (enabled: boolean) => void;
  setBasis: (basis: Basis) => void;
  setAnimationToVideo: (value: "none" | "mp4" | "webm") => void;
  setAnimationTargetBytes: (value: number | null) => void;
  setProcessing: (patch: Partial<Settings["processing"]>) => void;
  setOutput: (patch: Partial<Settings["output"]>) => void;

  setGlobalSetting: (setting: Setting | null) => void;
  setGroupTier: (group: string, tier: GroupTierState) => void;
  setFileSetting: (id: string, setting: Setting | null) => void;
  clearAllSettings: () => void;

  refreshPlan: () => Promise<void>;
  execute: () => Promise<void>;
  checkFfmpeg: () => Promise<void>;
}

function nowTime(): string {
  return new Date().toLocaleTimeString("zh-CN", { hour12: false });
}

export const useStore = create<MeowState>((set, get) => ({
  info: null,
  settings: null,
  resolvedTheme: "light",
  backgroundUrl: null,
  backgroundPath: null,
  aboutOpen: false,
  appearanceOpen: false,
  layout: DEFAULT_LAYOUT,

  root: null,
  scanOptions: { recursive: true, include: [], exclude: [] },
  files: [],
  outputDir: null,
  scanning: false,

  grouping: "prefix",
  groups: [],
  groupsDirty: false,
  pendingGrouping: null,

  globalSetting: null,
  groupTiers: {},
  fileSettings: {},

  linkEnabled: true,
  basis: "selection",
  animationToVideo: "none",
  animationTargetBytes: null,

  scope: { type: "global" },
  selectedIds: [],

  plan: null,
  report: null,
  running: false,
  ffmpegCheck: null,
  logs: [],

  log: (level, message) => {
    set((state) => {
      const logs = [...state.logs, { time: nowTime(), level, message }];
      return { logs: logs.length > MAX_LOGS ? logs.slice(-MAX_LOGS) : logs };
    });
  },

  init: async () => {
    try {
      const [info, settings] = await Promise.all([api.appInfo(), api.loadSettings()]);
      set({
        info,
        settings,
        grouping: settings.grouping,
        // 老配置没有 layout 段时，用旧的四个尺寸推出等价排布
        layout:
          parseLayout(settings.theme.layout) ??
          fromLegacy(
            settings.theme.sidebarWidth,
            settings.theme.previewWidth,
            settings.theme.runHeight,
            settings.theme.logHeight,
          ),
      });
      await get().applyAppearance();

      get().log("INFO", `${info.name} v${info.version} 已启动`);
      get().log("INFO", `配置目录：${info.configDir}（${info.configMode}）`);
      if (!info.configPersistent) {
        get().log("WARN", "配置未能持久化：当前环境不可写，已降级到临时目录");
      }

      await get().checkFfmpeg();
    } catch (error) {
      get().log("ERROR", `初始化失败：${String(error)}`);
    }
  },

  /** 把外观设置落到 DOM：主题模式、界面密度、主题色派生出的整组颜色。 */
  applyAppearance: async () => {
    const settings = get().settings;
    if (!settings) return;

    const mode = settings.theme.mode;
    const systemDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
    const resolved: "light" | "dark" = mode === "system" ? (systemDark ? "dark" : "light") : mode;

    const root = document.documentElement;
    root.dataset.theme = resolved;
    root.dataset.density = settings.theme.density;

    const palette = accentPalette(settings.theme.accent, resolved);
    root.style.setProperty("--accent", palette.accent);
    root.style.setProperty("--accent-hover", palette.hover);
    root.style.setProperty("--accent-soft", palette.soft);
    root.style.setProperty("--accent-contrast", palette.contrast);

    set({ resolvedTheme: resolved });

    // 背景图按路径读取一次，转成 data URL 供界面渲染。
    // 路径没变就不重复读取——否则每次系统主题切换都会把整张图片重新读一遍并编码。
    const path = settings.theme.backgroundImage;
    if (!path) {
      set({ backgroundUrl: null, backgroundPath: null });
      return;
    }
    if (path === get().backgroundPath && get().backgroundUrl) return;

    try {
      set({ backgroundUrl: await api.readBackgroundImage(path), backgroundPath: path });
    } catch (error) {
      set({ backgroundUrl: null, backgroundPath: null });
      get().log("WARN", `背景图无法加载：${String(error)}`);
    }
  },

  /** 修改外观设置并持久化。 */
  setTheme: async (patch) => {
    const settings = get().settings;
    if (!settings) return;
    const next: Settings = { ...settings, theme: { ...settings.theme, ...patch } };
    set({ settings: next });
    await get().applyAppearance();
    try {
      await api.saveSettings(next);
    } catch (error) {
      get().log("ERROR", `保存外观设置失败：${String(error)}`);
    }
  },

  setAboutOpen: (open) => set({ aboutOpen: open }),

  setAppearanceOpen: (open) => set({ appearanceOpen: open }),

  /** 应用新的分区布局并持久化。 */
  applyLayout: (next) => {
    set({ layout: next });
    const settings = get().settings;
    if (!settings) return;
    const updated: Settings = {
      ...settings,
      theme: { ...settings.theme, layout: next },
    };
    set({ settings: updated });
    api
      .saveSettings(updated)
      .catch((error) => get().log("ERROR", `保存布局失败：${String(error)}`));
  },

  setScanOptions: (patch) => set((state) => ({ scanOptions: { ...state.scanOptions, ...patch } })),

  scanFolder: async (path, options) => {
    const scanOptions = { ...get().scanOptions, ...options };
    set({ scanning: true, scanOptions });
    try {
      const result = await api.scanFolder(path, scanOptions);
      const grouping = get().grouping;
      const groups = await api.groupEntries(
        result.files.map((f) => ({ id: f.id, name: f.name, relativeParent: f.relativeParent })),
        grouping,
      );

      const known = new Set(result.files.map((f) => f.id));
      set((state) => {
        const fileSettings: Record<string, Setting> = {};
        for (const [id, setting] of Object.entries(state.fileSettings)) {
          if (known.has(id)) fileSettings[id] = setting;
        }
        return {
          root: result.root,
          files: result.files,
          outputDir: result.outputDir,
          groups,
          groupsDirty: false,
          pendingGrouping: null,
          groupTiers: {},
          fileSettings,
          report: null,
          selectedIds: [],
          scope: { type: "global" as const },
          scanning: false,
        };
      });

      const skipped = result.files.filter((f) => f.skipReason !== null).length;
      get().log("INFO", `扫描完成：共 ${result.files.length} 个文件，其中 ${skipped} 个已跳过`);
      get().log("INFO", `输出目录：${result.outputDir}`);

      // 最近使用的文件夹（最多 10 条）持久化保存
      try {
        set({ settings: await api.touchRecentFolder(result.root) });
      } catch (error) {
        get().log("WARN", `保存最近文件夹失败：${String(error)}`);
      }

      await get().refreshPlan();
    } catch (error) {
      set({ scanning: false });
      get().log("ERROR", `扫描失败：${String(error)}`);
    }
  },

  requestGrouping: (grouping) => {
    if (grouping === get().grouping) return;
    // 已有手动修正时先挂起，等界面确认（规范 6.4）
    set({ pendingGrouping: grouping });
    if (get().groupsDirty) return;
    void get().confirmGrouping();
  },

  confirmGrouping: async () => {
    const grouping = get().pendingGrouping;
    if (!grouping) return;
    try {
      const groups = await api.groupEntries(
        get().files.map((f) => ({ id: f.id, name: f.name, relativeParent: f.relativeParent })),
        grouping,
      );
      set({
        grouping,
        groups,
        groupsDirty: false,
        pendingGrouping: null,
        groupTiers: {},
        scope: { type: "global" },
      });
      get().log("INFO", `分组方式已切换为「${grouping}」，手动修正已重置`);
      await get().refreshPlan();
    } catch (error) {
      get().log("ERROR", `重新分组失败：${String(error)}`);
    }
  },

  cancelGrouping: () => set({ pendingGrouping: null }),

  renameGroup: (from, to) => {
    const name = to.trim();
    if (!name || name === from) return;
    set((state) => {
      const groupTiers = { ...state.groupTiers };
      if (from in groupTiers) {
        groupTiers[name] = groupTiers[from];
        delete groupTiers[from];
      }
      return {
        groups: coalesce(state.groups.map((g) => (g.name === from ? { ...g, name } : g))),
        groupsDirty: true,
        groupTiers,
      };
    });
    get().log("INFO", `分组已重命名：${from} → ${name}`);
    void get().refreshPlan();
  },

  moveFile: (fileId, toGroup) => {
    set((state) => {
      if (!state.groups.some((g) => g.name === toGroup)) return state;
      const groups = state.groups.map((g) => ({
        ...g,
        fileIds: g.fileIds.filter((id) => id !== fileId),
      }));
      const target = groups.find((g) => g.name === toGroup);
      if (target) target.fileIds = [...target.fileIds, fileId];
      return { groups, groupsDirty: true };
    });
    get().log("INFO", `已把 ${fileId} 移动到分组「${toGroup}」`);
    void get().refreshPlan();
  },

  mergeGroups: (from, into) => {
    if (from === into) return;
    set((state) => {
      const moving = state.groups.find((g) => g.name === from);
      if (!moving) return state;
      const groupTiers = { ...state.groupTiers };
      delete groupTiers[from];
      return {
        groups: coalesce(
          state.groups
            .filter((g) => g.name !== from)
            .map((g) => (g.name === into ? { ...g, fileIds: [...g.fileIds, ...moving.fileIds] } : g)),
        ),
        groupsDirty: true,
        groupTiers,
      };
    });
    get().log("INFO", `分组已合并：${from} → ${into}`);
    void get().refreshPlan();
  },

  setScope: (scope) => set({ scope }),

  toggleSelect: (id, additive) => {
    set((state) => {
      if (!additive) return { selectedIds: [id] };
      const has = state.selectedIds.includes(id);
      return {
        selectedIds: has ? state.selectedIds.filter((x) => x !== id) : [...state.selectedIds, id],
      };
    });
    void get().refreshPlan();
  },

  setLinkEnabled: (enabled) => {
    set({ linkEnabled: enabled });
    get().log(
      "INFO",
      enabled
        ? "已开启「按比例自动计算」：只填一边将按基准尺寸自动补全另一边"
        : "已关闭「按比例自动计算」：宽高各自独立，只填一边将报错",
    );
    void get().refreshPlan();
  },

  setBasis: (basis) => {
    set({ basis });
    void get().refreshPlan();
  },

  setAnimationToVideo: (value) => {
    set({ animationToVideo: value });
    get().log("INFO", value === "none" ? "动图保持原格式输出" : `动图将转为 ${value.toUpperCase()}`);
  },

  setAnimationTargetBytes: (value) => set({ animationTargetBytes: value }),

  setProcessing: (patch) => {
    const settings = get().settings;
    if (!settings) return;
    const next: Settings = { ...settings, processing: { ...settings.processing, ...patch } };
    set({ settings: next });
    api.saveSettings(next).catch((error) => get().log("ERROR", `保存设置失败：${String(error)}`));
    void get().refreshPlan();
  },

  setOutput: (patch) => {
    const settings = get().settings;
    if (!settings) return;
    const next: Settings = { ...settings, output: { ...settings.output, ...patch } };
    set({ settings: next });
    api.saveSettings(next).catch((error) => get().log("ERROR", `保存设置失败：${String(error)}`));
    void get().refreshPlan();
  },

  setGlobalSetting: (setting) => {
    set({ globalSetting: setting });
    void get().refreshPlan();
  },

  setGroupTier: (group, tier) => {
    set((state) => ({ groupTiers: { ...state.groupTiers, [group]: tier } }));
    void get().refreshPlan();
  },

  setFileSetting: (id, setting) => {
    set((state) => {
      const fileSettings = { ...state.fileSettings };
      if (setting === null) delete fileSettings[id];
      else fileSettings[id] = setting;
      return { fileSettings };
    });
    void get().refreshPlan();
  },

  clearAllSettings: () => {
    set({ globalSetting: null, groupTiers: {}, fileSettings: {} });
    get().log("INFO", "已清空全部缩放设置，所有素材将保持原样");
    void get().refreshPlan();
  },

  refreshPlan: async () => {
    const state = get();
    if (state.files.length === 0) {
      set({ plan: null });
      return;
    }
    try {
      const { request } = buildRequest(state);
      set({ plan: await api.previewPlan(request) });
    } catch (error) {
      get().log("ERROR", `预览计算失败：${String(error)}`);
    }
  },

  /** FFmpeg 自检：缺失的组件必须**告知用户**，而不是删掉对应功能后静默发布（规范 12.5）。 */
  checkFfmpeg: async () => {
    try {
      const check = await api.ffmpegSelfCheck();
      set({ ffmpegCheck: check });
      if (!check.ffmpegFound) {
        get().log("WARN", "未找到 FFmpeg：视频与动图将无法处理。请按 README 放置 ffmpeg.exe 与 ffprobe.exe");
        return;
      }
      const missing = [
        ...check.missingEncoders.map((e) => `编码器 ${e}`),
        ...check.missingFilters.map((f) => `滤镜 ${f}`),
        ...(check.missingHevcDecoder ? ["HEVC 解码器"] : []),
      ];
      if (missing.length > 0) {
        get().log("WARN", `FFmpeg 构建缺少：${missing.join("、")}`);
      } else {
        get().log("INFO", "FFmpeg 自检通过：所需编码器、滤镜与解码器齐备");
      }
    } catch (error) {
      get().log("WARN", `FFmpeg 自检失败：${String(error)}`);
    }
  },

  execute: async () => {
    const state = get();
    if (!state.root) return;
    if (!state.plan) return;

    // 校验失败时不得写出任何文件（规范 13.1 的 VALIDATION_FAILED）
    if (!state.plan.ok) {
      get().log("ERROR", "存在校验失败的素材，已阻止执行。请先修正后再执行。");
      return;
    }

    set({ running: true, report: null });
    try {
      const { request, sources } = buildRequest(state);
      const options = {
        outputDir: null,
        keepStructure: state.settings?.output.keepStructure ?? true,
        onConflict: state.settings?.output.onConflict ?? ("skip" as const),
        image: {
          resample: state.settings?.processing.resample ?? ("lanczos3" as const),
          quality: state.settings?.processing.jpgQuality ?? 85,
          format: state.settings?.output.outputFormat ?? ("keep" as const),
          keepAllMetadata: !(state.settings?.output.stripRedundantMetadata ?? true),
          backgroundFill: state.settings?.output.backgroundFillColor ?? "#FFFFFF",
        },
        animation: {
          colors: state.settings?.processing.gifColors ?? 256,
          dither: state.settings?.processing.gifDither ?? false,
          toVideo: state.animationToVideo,
          targetBytes: state.animationTargetBytes,
        },
        video: {
          codec: (state.settings?.processing.videoEncoder ?? "h264") as VideoCodec,
          crf: state.settings?.processing.videoCrf ?? 23,
          preset: "medium",
          hardware: state.settings?.processing.hardwareAccel ?? false,
          accel: state.settings?.processing.videoAccel ?? ("nvenc" as const),
          tonemapToSdr: state.settings?.processing.hdrTonemapToSdr ?? false,
        },
      };

      get().log("INFO", `开始执行：共 ${request.files.length} 个素材`);
      const report = await api.executePlan(request, sources, options, state.root);
      set({ report, running: false });

      const c = report.counts;
      get().log(
        "INFO",
        `执行完成：成功 ${c.success}、未改动 ${c.unchanged}、已跳过 ${c.skipped}、失败 ${c.failed}`,
      );
      get().log("INFO", `输出目录：${report.outputDir}`);
      // 运行级说明，例如硬件编码失败后的回退（规范 6.9：回退行为须记录在日志中）
      for (const note of report.notes ?? []) get().log("WARN", note);
      if (c.failed > 0) {
        get().log("WARN", `有 ${c.failed} 个素材处理失败，详情见结果列表`);
      }
    } catch (error) {
      set({ running: false });
      get().log("ERROR", `执行失败：${String(error)}`);
    }
  },
}));
