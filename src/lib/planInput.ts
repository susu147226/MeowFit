/** 由界面状态构建任务计划请求。
 *
 * 这一层是纯函数、不依赖 Tauri，因此可以单独测试——报错「只填了一边」这类
 * 问题都出在这里，必须能复现才好改。
 */

import { linkDimension } from "./expression";
import type {
  Group,
  GroupTierState,
  PlanFileInput,
  PlanGroupInput,
  PlanRequest,
  ScannedFile,
  Setting,
  Settings,
  SourceRef,
} from "../types";

export type Scope =
  | { type: "global" }
  | { type: "group"; name: string }
  | { type: "file"; id: string };

/** 「按比例自动计算」的基准来源 */
export type Basis = "selection" | "groupMax" | "groupMin";

export interface PlanState {
  files: ScannedFile[];
  groups: Group[];
  scope: Scope;
  selectedIds: string[];
  basis: Basis;
  linkEnabled: boolean;
  globalSetting: Setting | null;
  groupTiers: Record<string, GroupTierState>;
  fileSettings: Record<string, Setting>;
  settings: Settings | null;
}

/** 可参与处理的素材：有尺寸、且扫描阶段未被标记跳过。 */
export function processable(files: ScannedFile[]): ScannedFile[] {
  return files.filter((f) => f.skipReason === null && f.width !== null && f.height !== null);
}

/**
 * 参与计算的目标基准尺寸（规范 10.5）。
 *
 * 未声明尺寸的 SVG 且用户选择「按 DPI 换算」时，基准为 viewBox 尺寸 × dpi ÷ 96。
 */
export function dimsOf(
  file: Pick<ScannedFile, "width" | "height" | "kind" | "svgDeclared">,
  settings: Settings | null,
): { width: number; height: number } {
  const width = file.width ?? 0;
  const height = file.height ?? 0;
  if (file.kind === "svg" && !file.svgDeclared && settings?.processing.svgSizeMode === "dpi") {
    const scale = (settings.processing.svgDpi || 96) / 96;
    return { width: Math.round(width * scale), height: Math.round(height * scale) };
  }
  return { width, height };
}

/** 依据作用域与基准选项，取出用于「按比例自动计算」的参考尺寸（规范 6.3）。 */
export function referenceFor(state: PlanState, scope: Scope): { width: number; height: number } {
  if (scope.type === "file") {
    const file = state.files.find((f) => f.id === scope.id);
    if (file?.width && file?.height) return dimsOf(file, state.settings);
  }

  const groupFiles =
    scope.type === "group"
      ? state.groups.find((g) => g.name === scope.name)?.fileIds ?? []
      : null;

  const pool = processable(state.files).filter((f) =>
    groupFiles ? groupFiles.includes(f.id) : true,
  );

  if (state.basis === "selection") {
    const selected = pool.find((f) => state.selectedIds.includes(f.id));
    if (selected) return dimsOf(selected, state.settings);
  } else if (pool.length > 0) {
    const sizes = pool.map((f) => {
      const d = dimsOf(f, state.settings);
      return d.width * d.height;
    });
    const target = state.basis === "groupMax" ? Math.max(...sizes) : Math.min(...sizes);
    const picked = pool[pool.findIndex((f) => dimsOf(f, state.settings).width * dimsOf(f, state.settings).height === target)];
    if (picked) return dimsOf(picked, state.settings);
  }

  const fallback = pool[0];
  if (fallback) return dimsOf(fallback, state.settings);
  return { width: 0, height: 0 };
}

/** 该设置里是否有可用于计算的参数。 */
export function hasUsableParams(setting: Setting): boolean {
  switch (setting.mode) {
    case "A":
    case "G":
      return typeof setting.scale === "number" && setting.scale > 0;
    case "B":
    case "C":
    case "D":
      return typeof setting.width === "number" || typeof setting.height === "number";
    case "E":
      return typeof setting.limit === "number";
  }
}

/**
 * 把「只填了一边」的参数补齐（规范 6.3 的「按比例自动计算」）。
 *
 * 返回 `null` 表示**该层视为「未设置」**，两种情况：
 *   1. 用户还没填任何参数（只是切换到某个方式）——不该因此报错；
 *   2. 联动开着、但当前作用域没有任何可用基准素材，推不出另一边——
 *      此时也不能把「只有一边」的设置发下去，否则每个文件都会报缺一边。
 *
 * 联动**关闭**时不补齐，原样交给 Rust，由它按 13.2 报 `E_INCOMPLETE_DIMENSION`
 * 并指明缺哪一边——这是规范要求的行为。
 */
export function materialize(
  setting: Setting,
  reference: { width: number; height: number },
  linkEnabled: boolean,
): Setting | null {
  if (!hasUsableParams(setting)) return null;
  if (setting.mode !== "B" && setting.mode !== "C" && setting.mode !== "D") return setting;
  if (!linkEnabled) return setting;

  const linked = linkDimension(reference, setting.width ?? null, setting.height ?? null);
  if (linked.width === null || linked.height === null) return null;
  return { ...setting, width: linked.width, height: linked.height };
}

/** 由当前界面状态构建任务计划请求。预览与执行共用，保证两者看到的是同一套参数。 */
export function buildRequest(state: PlanState): { request: PlanRequest; sources: SourceRef[] } {
  const files = processable(state.files);
  const link = state.linkEnabled;

  const groupOf = new Map<string, string>();
  for (const group of state.groups) {
    for (const id of group.fileIds) groupOf.set(id, group.name);
  }

  const globalRef = referenceFor(state, state.scope);
  const global = state.globalSetting
    ? materialize(state.globalSetting, globalRef, link)
    : null;

  const planFiles: PlanFileInput[] = files.map((f) => {
    const own = state.fileSettings[f.id];
    const group = groupOf.get(f.id) ?? "";
    const setting = own
      ? materialize(own, referenceFor(state, { type: "group", name: group }), link)
      : null;
    const dims = dimsOf(f, state.settings);
    return {
      id: f.id,
      name: f.name,
      width: dims.width,
      height: dims.height,
      isVideo: f.kind === "video",
      group,
      setting,
    };
  });

  const planGroups: PlanGroupInput[] = state.groups.map((g) => {
    const tier = state.groupTiers[g.name] ?? null;
    if (tier && tier.kind === "explicit") {
      const ref = referenceFor(state, { type: "group", name: g.name });
      const setting = materialize(tier.setting, ref, link);
      return { name: g.name, setting: setting ? { kind: "explicit", setting } : null };
    }
    return { name: g.name, setting: tier };
  });

  const sources: SourceRef[] = files.map((f) => ({
    id: f.id,
    path: f.path,
    kind: f.kind as SourceRef["kind"],
  }));

  return { request: { files: planFiles, groups: planGroups, global }, sources };
}

/** 合并同名分组，避免重命名 / 合并后出现两组同名。 */
export function coalesce(groups: Group[]): Group[] {
  const merged: Group[] = [];
  for (const group of groups) {
    const existing = merged.find((g) => g.name === group.name);
    if (existing) {
      existing.fileIds = [...new Set([...existing.fileIds, ...group.fileIds])];
    } else {
      merged.push({ ...group, fileIds: [...group.fileIds] });
    }
  }
  return merged;
}
