import { describe, expect, it } from "vitest";

import {
  equivalentScale,
  linkDimension,
  parseDimension,
  parseNumberInput,
  parseScale,
} from "../../src/lib/expression";

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
