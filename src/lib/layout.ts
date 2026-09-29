/** 分区布局：用户可以自由拖动各分区，把主工作区排成自己想要的形状。
 *
 * 模型：主工作区由若干**列**组成，每列里纵向堆叠若干**分区**；列宽可拖动，
 * 列内第一个分区自适应剩余高度，其余分区用固定高度。日志区固定在底部停靠。
 */

export type PanelId = "files" | "settings" | "preview" | "run";

export interface LayoutColumn {
  id: string;
  panels: PanelId[];
  /** `null` 表示自适应剩余宽度 */
  width: number | null;
  /** 列内第二个及之后分区的高度 */
  tailHeight: number;
}

export interface Layout {
  columns: LayoutColumn[];
  /** 底部日志区的高度 */
  logHeight: number;
}

export const ALL_PANELS: PanelId[] = ["files", "settings", "preview", "run"];

export const PANEL_TITLE: Record<PanelId, string> = {
  files: "素材列表区",
  settings: "参数设置区",
  preview: "预览区",
  run: "执行与进度区",
};

export const COLUMN_MIN_WIDTH = 240;
export const COLUMN_MAX_WIDTH = 720;
export const TAIL_MIN_HEIGHT = 120;
export const TAIL_MAX_HEIGHT = 620;
export const LOG_MIN_HEIGHT = 80;
export const LOG_MAX_HEIGHT = 420;

const DEFAULT_TAIL_HEIGHT = 280;

export const DEFAULT_LAYOUT: Layout = {
  columns: [
    { id: "col-files", panels: ["files"], width: null, tailHeight: DEFAULT_TAIL_HEIGHT },
    { id: "col-settings", panels: ["settings"], width: 350, tailHeight: DEFAULT_TAIL_HEIGHT },
    { id: "col-preview", panels: ["preview", "run"], width: 430, tailHeight: DEFAULT_TAIL_HEIGHT },
  ],
  logHeight: 190,
};

/**
 * 把布局规整成合法状态。
 *
 * 保证：每个分区**恰好出现一次**（拖动过程中难免出现重复或丢失），空列被移除。
 * 少了的分区补回第一列，避免用户误操作把某个分区拖没了又找不回来。
 */
export function normalize(layout: Layout): Layout {
  const seen = new Set<PanelId>();
  const columns: LayoutColumn[] = [];

  for (const column of layout.columns) {
    // 逐个判定并即时记入 seen，否则同一列内部的重复会被同时放行
    const panels: PanelId[] = [];
    for (const panel of column.panels) {
      if (!ALL_PANELS.includes(panel) || seen.has(panel)) continue;
      seen.add(panel);
      panels.push(panel);
    }
    if (panels.length > 0) {
      columns.push({
        id: column.id,
        panels,
        width: column.width,
        tailHeight: column.tailHeight || DEFAULT_TAIL_HEIGHT,
      });
    }
  }

  const missing = ALL_PANELS.filter((p) => !seen.has(p));
  if (missing.length > 0) {
    if (columns.length === 0) {
      columns.push({ id: "col-main", panels: [], width: 350, tailHeight: DEFAULT_TAIL_HEIGHT });
    }
    columns[0] = { ...columns[0], panels: [...columns[0].panels, ...missing] };
  }

  const logHeight = Math.max(LOG_MIN_HEIGHT, Math.min(LOG_MAX_HEIGHT, layout.logHeight));
  return { columns, logHeight };
}

/** 把分区移动到某一列的指定位置。 */
export function movePanel(
  layout: Layout,
  panelId: PanelId,
  targetColumnId: string,
  index: number,
): Layout {
  if (!layout.columns.some((c) => c.id === targetColumnId)) return layout;
  const columns = layout.columns.map((c) => ({
    ...c,
    panels: c.panels.filter((p) => p !== panelId),
  }));
  const target = columns.find((c) => c.id === targetColumnId);
  if (!target) return layout;
  target.panels.splice(Math.max(0, Math.min(index, target.panels.length)), 0, panelId);
  return normalize({ ...layout, columns });
}

/** 新建一列并把分区放进去；`atIndex` 是插入到第几列之前。 */
export function movePanelToNewColumn(layout: Layout, panelId: PanelId, atIndex: number): Layout {
  const columns = layout.columns.map((c) => ({
    ...c,
    panels: c.panels.filter((p) => p !== panelId),
  }));

  const next: LayoutColumn[] = [];
  let inserted = false;
  for (let i = 0; i < columns.length; i += 1) {
    if (i === atIndex) {
      next.push({
        id: `col-${panelId}-${i}`,
        panels: [panelId],
        width: 350,
        tailHeight: DEFAULT_TAIL_HEIGHT,
      });
      inserted = true;
    }
    next.push(columns[i]);
  }
  if (!inserted) {
    next.push({
      id: `col-${panelId}-end`,
      panels: [panelId],
      width: 350,
      tailHeight: DEFAULT_TAIL_HEIGHT,
    });
  }
  return normalize({ ...layout, columns: next });
}

/** 分区当前所在的列与列内位置。 */
export function locate(
  layout: Layout,
  panelId: PanelId,
): { columnId: string; index: number } | null {
  for (const column of layout.columns) {
    const index = column.panels.indexOf(panelId);
    if (index >= 0) return { columnId: column.id, index };
  }
  return null;
}

export function setColumnWidth(layout: Layout, columnId: string, width: number | null): Layout {
  const clamped =
    width === null ? null : Math.max(COLUMN_MIN_WIDTH, Math.min(COLUMN_MAX_WIDTH, width));
  return { ...layout, columns: layout.columns.map((c) => (c.id === columnId ? { ...c, width: clamped } : c)) };
}

export function setColumnTailHeight(layout: Layout, columnId: string, height: number): Layout {
  const clamped = Math.max(TAIL_MIN_HEIGHT, Math.min(TAIL_MAX_HEIGHT, height));
  return {
    ...layout,
    columns: layout.columns.map((c) => (c.id === columnId ? { ...c, tailHeight: clamped } : c)),
  };
}

export function setLogHeight(layout: Layout, height: number): Layout {
  return { ...layout, logHeight: Math.max(LOG_MIN_HEIGHT, Math.min(LOG_MAX_HEIGHT, height)) };
}

/** 由旧的四个尺寸推出等价布局，保证老配置升级后观感不变。 */
export function fromLegacy(
  sidebarWidth: number,
  previewWidth: number,
  runHeight: number,
  logHeight: number,
): Layout {
  return normalize({
    columns: [
      { id: "col-files", panels: ["files"], width: null, tailHeight: runHeight },
      { id: "col-settings", panels: ["settings"], width: sidebarWidth, tailHeight: runHeight },
      { id: "col-preview", panels: ["preview", "run"], width: previewWidth, tailHeight: runHeight },
    ],
    logHeight,
  });
}

/** 解析持久化下来的布局；结构不合法时返回 `null`，由调用方退回默认。 */
export function parseLayout(raw: unknown): Layout | null {
  if (!raw || typeof raw !== "object") return null;
  const value = raw as { columns?: unknown; logHeight?: unknown };
  if (!Array.isArray(value.columns) || value.columns.length === 0) return null;

  const columns: LayoutColumn[] = [];
  for (const item of value.columns) {
    if (!item || typeof item !== "object") return null;
    const column = item as { id?: unknown; panels?: unknown; width?: unknown; tailHeight?: unknown };
    if (typeof column.id !== "string" || !Array.isArray(column.panels)) return null;
    const panels = column.panels.filter((p): p is PanelId => typeof p === "string" && ALL_PANELS.includes(p as PanelId));
    const width =
      typeof column.width === "number" && Number.isFinite(column.width) ? column.width : null;
    const tailHeight = typeof column.tailHeight === "number" ? column.tailHeight : DEFAULT_TAIL_HEIGHT;
    columns.push({ id: column.id, panels, width, tailHeight });
  }
  if (columns.every((c) => c.panels.length === 0)) return null;

  const logHeight = typeof value.logHeight === "number" ? value.logHeight : DEFAULT_LAYOUT.logHeight;
  return normalize({ columns, logHeight });
}
