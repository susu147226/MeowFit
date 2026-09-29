import { describe, expect, it } from "vitest";

import { accentPalette, contrastRatio, mix, parseHex, readableOn, toHex } from "../../src/lib/color";

describe("颜色解析与转换", () => {
  it("接受 #RRGGBB 与 #RGB", () => {
    expect(parseHex("#A8763A")).toEqual({ r: 168, g: 118, b: 58 });
    expect(parseHex("A8763A")).toEqual({ r: 168, g: 118, b: 58 });
    expect(parseHex("#abc")).toEqual({ r: 170, g: 187, b: 204 });
  });

  it("非法输入返回 null", () => {
    expect(parseHex("")).toBeNull();
    expect(parseHex("#12345")).toBeNull();
    expect(parseHex("红色")).toBeNull();
  });

  it("往返转换保持一致", () => {
    expect(toHex({ r: 168, g: 118, b: 58 })).toBe("#A8763A");
    // 越界值被夹到合法范围
    expect(toHex({ r: 300, g: -20, b: 128 })).toBe("#FF0080");
  });

  it("按比例混合", () => {
    expect(mix({ r: 0, g: 0, b: 0 }, { r: 100, g: 100, b: 100 }, 0.5)).toEqual({
      r: 50,
      g: 50,
      b: 50,
    });
  });
});

describe("对比度", () => {
  it("黑白对比度为 21", () => {
    expect(contrastRatio({ r: 0, g: 0, b: 0 }, { r: 255, g: 255, b: 255 })).toBeCloseTo(21, 1);
  });

  it("同色对比度为 1", () => {
    expect(contrastRatio({ r: 80, g: 80, b: 80 }, { r: 80, g: 80, b: 80 })).toBeCloseTo(1, 5);
  });

  it("在深色上选白色文字，浅色上选黑色文字", () => {
    expect(readableOn({ r: 20, g: 20, b: 20 })).toEqual({ r: 255, g: 255, b: 255 });
    expect(readableOn({ r: 240, g: 240, b: 240 })).toEqual({ r: 0, g: 0, b: 0 });
  });
});

describe("由主题色推导整套界面色", () => {
  it("过浅的主色在浅色模式下被压暗到可辨", () => {
    const palette = accentPalette("#FFF3E0", "light");
    const accent = parseHex(palette.accent)!;
    // 与浅色界面底色的对比度须达到 3:1
    expect(contrastRatio(accent, { r: 247, g: 246, b: 243 })).toBeGreaterThanOrEqual(3);
  });

  it("过暗的主色在深色模式下被提亮到可辨", () => {
    const palette = accentPalette("#101014", "dark");
    const accent = parseHex(palette.accent)!;
    expect(contrastRatio(accent, { r: 23, g: 22, b: 26 })).toBeGreaterThanOrEqual(3);
  });

  it("正常主色保持原值", () => {
    const palette = accentPalette("#A8763A", "light");
    expect(palette.accent).toBe("#A8763A");
  });

  it("派生色互不相同，且反白文字与主色有足够对比", () => {
    const palette = accentPalette("#2F6F4F", "light");
    expect(palette.hover).not.toBe(palette.accent);
    expect(palette.soft).not.toBe(palette.accent);
    // 主色按钮上的文字至少要达到大号文字的 3:1
    expect(
      contrastRatio(parseHex(palette.contrast)!, parseHex(palette.accent)!),
    ).toBeGreaterThanOrEqual(3);
  });

  it("非法输入退回默认主色而不是抛错", () => {
    const palette = accentPalette("not-a-color", "light");
    expect(parseHex(palette.accent)).not.toBeNull();
  });
});
