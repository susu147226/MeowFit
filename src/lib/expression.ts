/** 参数输入解析（规范 6.3）。
 *
 * 支持：`2`、`0.5`、`150%` 这类倍率写法，以及 `1920/2`、`1280*0.75` 这类算术表达式。
 * 只负责把输入变成数字；尺寸如何计算一律由 Rust 侧唯一实现决定。
 */

class ExprParser {
  private i = 0;

  constructor(private readonly src: string) {}

  parse(): number {
    const value = this.expr();
    this.skipWs();
    if (this.i < this.src.length) {
      throw new Error(`无法解析的字符：${this.src[this.i]}`);
    }
    return value;
  }

  private expr(): number {
    let value = this.term();
    for (;;) {
      this.skipWs();
      const c = this.src[this.i];
      if (c === "+") {
        this.i += 1;
        value += this.term();
      } else if (c === "-") {
        this.i += 1;
        value -= this.term();
      } else {
        return value;
      }
    }
  }

  private term(): number {
    let value = this.factor();
    for (;;) {
      this.skipWs();
      const c = this.src[this.i];
      if (c === "*") {
        this.i += 1;
        value *= this.factor();
      } else if (c === "/") {
        this.i += 1;
        const divisor = this.factor();
        if (divisor === 0) throw new Error("除数不能为 0");
        value /= divisor;
      } else {
        return value;
      }
    }
  }

  private factor(): number {
    this.skipWs();
    const c = this.src[this.i];

    if (c === "(") {
      this.i += 1;
      const value = this.expr();
      this.skipWs();
      if (this.src[this.i] !== ")") throw new Error("缺少右括号");
      this.i += 1;
      return value;
    }

    if (c === "-" || c === "+") {
      this.i += 1;
      const value = this.factor();
      return c === "-" ? -value : value;
    }

    return this.number();
  }

  private number(): number {
    const start = this.i;
    while (this.i < this.src.length && /[0-9.]/.test(this.src[this.i])) {
      this.i += 1;
    }
    if (this.i === start) throw new Error("此处需要一个数字");

    const value = Number(this.src.slice(start, this.i));
    if (!Number.isFinite(value)) throw new Error("非法数字");

    // 百分号按倍率处理：150% → 1.5
    this.skipWs();
    if (this.src[this.i] === "%") {
      this.i += 1;
      return value / 100;
    }
    return value;
  }

  private skipWs(): void {
    while (this.i < this.src.length && /\s/.test(this.src[this.i])) {
      this.i += 1;
    }
  }
}

/** 解析成功返回数值，失败返回 `null`（界面据此提示输入无效）。 */
export function parseNumberInput(raw: string): number | null {
  const source = raw.trim();
  if (source === "") return null;
  try {
    const value = new ExprParser(source).parse();
    return Number.isFinite(value) ? value : null;
  } catch {
    return null;
  }
}

/** 倍率输入的允许范围（规范 6.2：0.01–64），越界由界面拦截。 */
export const MIN_SCALE = 0.01;
export const MAX_SCALE = 64;

/** 尺寸上限（规范 6.2）。 */
export const MAX_DIMENSION = 32768;

export interface ParsedScale {
  value: number | null;
  /** 界面提示文案；`null` 表示输入合法 */
  error: string | null;
}

export function parseScale(raw: string): ParsedScale {
  const value = parseNumberInput(raw);
  if (value === null) return { value: null, error: "请输入数字、百分比或算术表达式" };
  if (value < MIN_SCALE || value > MAX_SCALE) {
    return { value: null, error: `倍率需在 ${MIN_SCALE}–${MAX_SCALE} 之间` };
  }
  return { value, error: null };
}

export interface ParsedDimension {
  value: number | null;
  error: string | null;
}

export function parseDimension(raw: string): ParsedDimension {
  const value = parseNumberInput(raw);
  if (value === null) return { value: null, error: "请输入数字或算术表达式" };
  if (value < 1) return { value: null, error: "尺寸不能小于 1px" };
  if (value > MAX_DIMENSION) return { value: null, error: `尺寸上限为 ${MAX_DIMENSION}` };
  return { value, error: null };
}

/** 按原始比例，由已填的一边算出另一边（规范 6.3 的「按比例自动计算」）。 */
export function linkDimension(
  reference: { width: number; height: number },
  width: number | null,
  height: number | null,
): { width: number | null; height: number | null } {
  if (width !== null && height === null && reference.width > 0) {
    return { width, height: Math.round((width * reference.height) / reference.width) };
  }
  if (height !== null && width === null && reference.height > 0) {
    return { width: Math.round((height * reference.width) / reference.height), height };
  }
  return { width, height };
}

/** 等效倍率：目标宽高相对基准尺寸的缩放倍数。 */
export function equivalentScale(
  reference: { width: number; height: number },
  width: number | null,
  height: number | null,
): number | null {
  if (reference.width <= 0 || reference.height <= 0) return null;
  if (width !== null) return width / reference.width;
  if (height !== null) return height / reference.height;
  return null;
}
