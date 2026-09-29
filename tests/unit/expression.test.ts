import { describe, expect, it } from "vitest";

import {
  equivalentScale,
  linkDimension,
  parseDimension,
  parseNumberInput,
  parseScale,
} from "../../src/lib/expression";
import { dimsOf } from "../../src/lib/planInput";

describe("parseNumberInput（规范 6.3 的参数输入）", () => {
  it("接受纯数字与小数", () => {
    expect(parseNumberInput("2")).toBe(2);
    expect(parseNumberInput("0.5")).toBe(0.5);
    expect(parseNumberInput("  3  ")).toBe(3);
  });

  it("接受百分比写法，按倍率换算", () => {
    expect(parseNumberInput("150%")).toBe(1.5);
    expect(parseNumberInput("50%")).toBe(0.5);
    expect(parseNumberInput("100 %")).toBe(1);
  });

  it("接受算术表达式", () => {
    expect(parseNumberInput("1920/2")).toBe(960);
    expect(parseNumberInput("1280*0.75")).toBe(960);
    expect(parseNumberInput("(800+400)/2")).toBe(600);
    expect(parseNumberInput("100-20")).toBe(80);
  });

  it("遵循先乘除后加减", () => {
    expect(parseNumberInput("2+3*4")).toBe(14);
    expect(parseNumberInput("10-6/3")).toBe(8);
  });

  it("非法输入返回 null", () => {
    expect(parseNumberInput("")).toBeNull();
    expect(parseNumberInput("abc")).toBeNull();
    expect(parseNumberInput("1920/")).toBeNull();
    expect(parseNumberInput("(1+2")).toBeNull();
    expect(parseNumberInput("1/0")).toBeNull();
    expect(parseNumberInput("1 2")).toBeNull();
  });
});

describe("parseScale（倍率范围 0.01–64，规范 6.2）", () => {
  it("范围内通过", () => {
    expect(parseScale("2").value).toBe(2);
    expect(parseScale("0.01").value).toBe(0.01);
    expect(parseScale("64").value).toBe(64);
    expect(parseScale("150%").value).toBe(1.5);
  });

  it("越界时给出提示且不返回值", () => {
    expect(parseScale("0.001").error).toContain("0.01");
    expect(parseScale("65").error).toContain("64");
    expect(parseScale("65").value).toBeNull();
  });
});

describe("parseDimension（尺寸上限 32768，规范 6.2）", () => {
  it("范围内通过", () => {
    expect(parseDimension("1920").value).toBe(1920);
    expect(parseDimension("1920/2").value).toBe(960);
  });

  it("小于 1px 或超过上限时报错", () => {
    expect(parseDimension("0").error).toContain("1px");
    expect(parseDimension("32769").error).toContain("32768");
  });
});

describe("按比例自动计算（规范 6.3）", () => {
  const reference = { width: 1920, height: 1080 };

  it("只填宽时按比例算出高", () => {
    expect(linkDimension(reference, 800, null)).toEqual({ width: 800, height: 450 });
  });

  it("只填高时按比例算出宽", () => {
    expect(linkDimension(reference, null, 540)).toEqual({ width: 960, height: 540 });
  });

  it("两边都填时不联动，原样返回", () => {
    expect(linkDimension(reference, 800, 800)).toEqual({ width: 800, height: 800 });
  });

  it("等效倍率按已填的一边计算", () => {
    expect(equivalentScale(reference, 960, null)).toBeCloseTo(0.5);
    expect(equivalentScale(reference, null, 540)).toBeCloseTo(0.5);
    expect(equivalentScale(reference, null, null)).toBeNull();
  });
});

describe("SVG 基准尺寸（规范 10.5）", () => {
  const svg = (declared: boolean) => ({
    width: 100,
    height: 50,
    kind: "svg" as const,
    svgDeclared: declared,
  });
  const withSvg = (mode: "pixel" | "dpi", dpi = 96) =>
    ({ processing: { svgSizeMode: mode, svgDpi: dpi } }) as never;

  it("已声明尺寸的 SVG 始终以声明值为基准，不受 DPI 选项影响", () => {
    expect(dimsOf(svg(true), withSvg("pixel"))).toEqual({ width: 100, height: 50 });
    expect(dimsOf(svg(true), withSvg("dpi", 192))).toEqual({ width: 100, height: 50 });
  });

  it("未声明尺寸时，pixel 模式以 viewBox 尺寸为基准", () => {
    expect(dimsOf(svg(false), withSvg("pixel", 192))).toEqual({ width: 100, height: 50 });
  });

  it("未声明尺寸时，dpi 模式按 × dpi ÷ 96 换算基准", () => {
    // 默认 96 DPI 即 1:1
    expect(dimsOf(svg(false), withSvg("dpi", 96))).toEqual({ width: 100, height: 50 });
    expect(dimsOf(svg(false), withSvg("dpi", 192))).toEqual({ width: 200, height: 100 });
    expect(dimsOf(svg(false), withSvg("dpi", 48))).toEqual({ width: 50, height: 25 });
  });

  it("非 SVG 素材不受该选项影响", () => {
    const png = { width: 640, height: 480, kind: "raster" as const, svgDeclared: false };
    expect(dimsOf(png, withSvg("dpi", 288))).toEqual({ width: 640, height: 480 });
  });

  it("缺少设置时退回未换算的尺寸", () => {
    expect(dimsOf(svg(false), null)).toEqual({ width: 100, height: 50 });
  });
});
