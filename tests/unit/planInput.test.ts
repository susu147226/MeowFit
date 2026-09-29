import { describe, expect, it } from "vitest";

import { buildRequest, materialize, processable, type PlanState } from "../../src/lib/planInput";
import type { ScannedFile, Setting } from "../../src/types";

/** 造一个和真实测试素材夹同构的状态：混合格式 + 三种前缀分组 + 若干跳过的动图。 */
function makeState(overrides: Partial<PlanState> = {}): PlanState {
  const file = (
    name: string,
    kind: ScannedFile["kind"],
    w: number | null,
    h: number | null,
    skipReason: string | null = null,
  ): ScannedFile => ({
    id: name,
    path: `D:\\素材\\${name}`,
    relativePath: name,
    relativeParent: name.includes("/") ? name.slice(0, name.lastIndexOf("/")) : "",
    name,
    ext: name.split(".").pop() ?? "",
    kind,
    size: 1000,
    mtimeMs: 1,
    width: w,
    height: h,
    svgDeclared: false,
    skipReason,
  });

  const files: ScannedFile[] = [
    file("jdt_1.png", "raster", 277, 280),
    file("jdt_2.png", "raster", 277, 280),
    file("num_0.png", "raster", 23, 31),
    file("num_1.png", "raster", 23, 31),
    file("ydy.png", "raster", 382, 599),
    file("bg_1.mp4", "video", 1080, 2640),
    // 动图尚未接入，扫描阶段即被跳过——它们不应产生任何报错
    file("gif/jdt_1.gif", "animated", null, null, "动图处理将在后续阶段接入"),
    file("gif/jdt_2.gif", "animated", null, null, "动图处理将在后续阶段接入"),
  ];

  return {
    files,
    groups: [
      { name: "bg", fileIds: ["bg_1.mp4"] },
      { name: "gif", fileIds: ["gif/jdt_1.gif", "gif/jdt_2.gif"] },
      { name: "jdt", fileIds: ["jdt_1.png", "jdt_2.png"] },
      { name: "num", fileIds: ["num_0.png", "num_1.png"] },
      { name: "ydy", fileIds: ["ydy.png"] },
    ],
    scope: { type: "global" },
    selectedIds: [],
    basis: "selection",
    linkEnabled: true,
    globalSetting: null,
    groupTiers: {},
    fileSettings: {},
    settings: null,
    ...overrides,
  };
}

const widthOnly: Setting = { mode: "B", width: 800 };

describe("只填了一边时的行为", () => {
  it("已被跳过的动图不进入计划，也不会产生报错", () => {
    const state = makeState({ globalSetting: widthOnly });
    const { request } = buildRequest(state);
    expect(request.files.map((f) => f.id)).not.toContain("gif/jdt_1.gif");
    expect(processable(state.files)).toHaveLength(6);
  });

  it("联动开启时，只填一边会按基准补齐另一边", () => {
    const state = makeState({ globalSetting: widthOnly, linkEnabled: true });
    const { request } = buildRequest(state);
    expect(request.global).not.toBeNull();
    expect(request.global?.width).toBe(800);
    expect(request.global?.height).toBeGreaterThan(0);
  });

  it("联动关闭时原样下发，交给引擎按 13.2 报错并指明缺哪一边", () => {
    const state = makeState({ globalSetting: widthOnly, linkEnabled: false });
    const { request } = buildRequest(state);
    expect(request.global?.width).toBe(800);
    expect(request.global?.height).toBeUndefined();
  });

  it("只是切换到某个方式、还没填任何参数时，视为未设置而不是报错", () => {
    // 这是用户遇到的主要问题：切到「自定义宽高」就开始报缺一边，且切换方式也不消失
    for (const mode of ["A", "B", "C", "D", "E", "G"] as const) {
      const state = makeState({ globalSetting: { mode } });
      const { request } = buildRequest(state);
      expect(request.global).toBeNull();
    }
  });

  it("联动开启但当前作用域没有可用基准时，视为未设置而不是每个文件都报错", () => {
    // 该分组里只有被跳过的动图
    const state = makeState({
      linkEnabled: true,
      groupTiers: { gif: { kind: "explicit", setting: widthOnly } },
    });
    const { request } = buildRequest(state);
    const gifGroup = request.groups.find((g) => g.name === "gif");
    expect(gifGroup?.setting).toBeNull();
  });

  it("materialize 对「没有任何参数」返回 null，对「填了一边且能补齐」返回完整设置", () => {
    const reference = { width: 277, height: 280 };
    expect(materialize({ mode: "B" }, reference, true)).toBeNull();
    expect(materialize({ mode: "A" }, reference, true)).toBeNull();
    expect(materialize({ mode: "A", scale: 2 }, reference, true)).toEqual({ mode: "A", scale: 2 });
    const filled = materialize({ mode: "B", width: 800 }, reference, true);
    expect(filled?.height).toBeGreaterThan(0);
  });
});

describe("分组设置只作用于该组", () => {
  it("只给 jdt 组设尺寸时，其余分组不受影响", () => {
    const state = makeState({
      groupTiers: { jdt: { kind: "explicit", setting: { mode: "D", width: 128, height: 128 } } },
    });
    const { request } = buildRequest(state);

    const jdt = request.files.filter((f) => f.group === "jdt");
    const others = request.files.filter((f) => f.group !== "jdt");
    expect(jdt).toHaveLength(2);
    expect(jdt.every((f) => f.setting === null)).toBe(true); // 组设置走 groups，不落到单文件

    // 组设置确实只挂在 jdt 组上
    const jdtGroup = request.groups.find((g) => g.name === "jdt");
    expect(jdtGroup?.setting).toEqual({
      kind: "explicit",
      setting: { mode: "D", width: 128, height: 128 },
    });
    const otherGroups = request.groups.filter((g) => g.name !== "jdt");
    expect(otherGroups.every((g) => g.setting === null)).toBe(true);

    // 整体未设置，因此除 jdt 外的素材全部保持原样
    expect(request.global).toBeNull();
    expect(others.every((f) => f.setting === null)).toBe(true);
  });

  it("单文件设置优先于分组设置", () => {
    const state = makeState({
      groupTiers: { jdt: { kind: "explicit", setting: { mode: "D", width: 128, height: 128 } } },
      fileSettings: { "jdt_1.png": { mode: "D", width: 256, height: 256 } },
    });
    const { request } = buildRequest(state);
    const single = request.files.find((f) => f.id === "jdt_1.png");
    expect(single?.setting).toEqual({ mode: "D", width: 256, height: 256 });
  });

  it("分组显式「跟随整体」时取整体设置", () => {
    const state = makeState({
      globalSetting: { mode: "A", scale: 2 },
      groupTiers: { jdt: { kind: "followGlobal" } },
    });
    const { request } = buildRequest(state);
    expect(request.groups.find((g) => g.name === "jdt")?.setting).toEqual({ kind: "followGlobal" });
    expect(request.global).toEqual({ mode: "A", scale: 2 });
  });
});

describe("界面布局作用域", () => {
  it("作用域为某分组时，该分组整体设置只影响该组的文件", () => {
    const state = makeState({
      scope: { type: "group", name: "num" },
      globalSetting: { mode: "A", scale: 0.5 },
    });
    const { request } = buildRequest(state);
    // scope 只影响「按比例自动计算」的基准，不改变设置归属
    expect(request.global).toEqual({ mode: "A", scale: 0.5 });
    expect(request.files.every((f) => f.setting === null)).toBe(true);
  });
});
