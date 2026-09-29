import { describe, expect, it } from "vitest";

import {
  COLUMN_MAX_WIDTH,
  COLUMN_MIN_WIDTH,
  DEFAULT_LAYOUT,
  fromLegacy,
  locate,
  movePanel,
  movePanelToNewColumn,
  normalize,
  parseLayout,
  setColumnWidth,
  setColumnTailHeight,
} from "../../src/lib/layout";

const ids = (layout: { columns: { panels: string[] }[] }) =>
  layout.columns.map((c) => c.panels.join("+"));

describe("布局规整", () => {
  it("每个分区恰好出现一次：重复的去掉，丢失的补回第一列", () => {
    const layout = normalize({
      columns: [
        { id: "a", panels: ["files", "files"], width: null, tailHeight: 280 },
        { id: "b", panels: ["settings"], width: 300, tailHeight: 280 },
      ],
      logHeight: 190,
    });
    expect(ids(layout)).toEqual(["files+preview+run", "settings"]);
  });

  it("空列被移除", () => {
    const layout = normalize({
      columns: [
        { id: "a", panels: ["files"], width: null, tailHeight: 280 },
        { id: "b", panels: [], width: 300, tailHeight: 280 },
        { id: "c", panels: ["preview"], width: 300, tailHeight: 280 },
      ],
      logHeight: 190,
    });
    // 空列被丢弃；缺的分区补进第一列
    expect(ids(layout)).toEqual(["files+settings+run", "preview"]);
  });

  it("全空的列会补出一个兜底列，不会把分区弄丢", () => {
    const layout = normalize({
      columns: [{ id: "a", panels: [], width: null, tailHeight: 280 }],
      logHeight: 190,
    });
    expect(layout.columns).toHaveLength(1);
    expect(layout.columns[0].panels).toHaveLength(4);
  });
});

describe("拖动分区", () => {
  it("把分区移入另一列的指定位置", () => {
    const next = movePanel(DEFAULT_LAYOUT, "files", "col-preview", 1);
    expect(ids(next)).toEqual(["settings", "preview+files+run"]);
  });

  it("把分区移到列首", () => {
    const next = movePanel(DEFAULT_LAYOUT, "run", "col-files", 0);
    expect(ids(next)).toEqual(["run+files", "settings", "preview"]);
  });

  it("从原列拖走后原列若空则被移除", () => {
    // settings 独占一列，拖到别处后该列应消失
    const next = movePanel(DEFAULT_LAYOUT, "settings", "col-files", 1);
    expect(ids(next)).toEqual(["files+settings", "preview+run"]);
  });

  it("拖到不存在的列时保持原样", () => {
    expect(ids(movePanel(DEFAULT_LAYOUT, "files", "不存在的列", 0))).toEqual(ids(DEFAULT_LAYOUT));
  });

  it("新建一列", () => {
    const next = movePanelToNewColumn(DEFAULT_LAYOUT, "run", 1);
    expect(ids(next)).toEqual(["files", "run", "settings", "preview"]);
  });

  it("拖到自己原本的位置不产生重复", () => {
    const next = movePanel(DEFAULT_LAYOUT, "files", "col-files", 0);
    expect(ids(next)).toEqual(ids(DEFAULT_LAYOUT));
  });

  it("locate 能报出分区所在的列与位置", () => {
    expect(locate(DEFAULT_LAYOUT, "run")).toEqual({ columnId: "col-preview", index: 1 });
    expect(locate(DEFAULT_LAYOUT, "files")).toEqual({ columnId: "col-files", index: 0 });
  });
});

describe("列宽与高度", () => {
  it("列宽被限制在合理范围", () => {
    const tooSmall = setColumnWidth(DEFAULT_LAYOUT, "col-settings", 10);
    const tooBig = setColumnWidth(DEFAULT_LAYOUT, "col-settings", 99999);
    expect(tooSmall.columns.find((c) => c.id === "col-settings")?.width).toBe(COLUMN_MIN_WIDTH);
    expect(tooBig.columns.find((c) => c.id === "col-settings")?.width).toBe(COLUMN_MAX_WIDTH);
  });

  it("列内高度可调且有限位", () => {
    const next = setColumnTailHeight(DEFAULT_LAYOUT, "col-preview", 999);
    expect(next.columns.find((c) => c.id === "col-preview")?.tailHeight).toBe(620);
  });
});

describe("与旧配置的兼容", () => {
  it("由旧的四个尺寸推出等价布局，老用户升级后观感不变", () => {
    const layout = fromLegacy(320, 460, 300, 210);
    expect(ids(layout)).toEqual(["files", "settings", "preview+run"]);
    expect(layout.columns[1].width).toBe(320);
    expect(layout.columns[2].width).toBe(460);
    expect(layout.columns[2].tailHeight).toBe(300);
    expect(layout.logHeight).toBe(210);
  });

  it("合法布局可解析", () => {
    const layout = parseLayout({
      columns: [{ id: "c1", panels: ["files", "settings"], width: null, tailHeight: 200 }],
      logHeight: 150,
    });
    expect(layout).not.toBeNull();
    // 未出现在配置里的分区会被补回第一列，保证不会丢
    expect(layout?.columns[0].panels).toEqual(["files", "settings", "preview", "run"]);
    expect(layout?.logHeight).toBe(150);
  });

  it("结构不合法时返回 null，由调用方退回默认", () => {
    expect(parseLayout(null)).toBeNull();
    expect(parseLayout({})).toBeNull();
    expect(parseLayout({ columns: [] })).toBeNull();
    expect(parseLayout({ columns: [{ panels: ["files"] }] })).toBeNull();
    expect(parseLayout({ columns: [{ id: "c", panels: ["不存在的分区"] }] })).toBeNull();
    expect(parseLayout("字符串")).toBeNull();
  });

  it("解析时会剔除不认识的分区名", () => {
    const layout = parseLayout({
      columns: [{ id: "c1", panels: ["files", "乱写的", "settings"], width: null, tailHeight: 200 }],
      logHeight: 150,
    });
    expect(layout?.columns[0].panels).toEqual(["files", "settings", "preview", "run"]);
  });
});
