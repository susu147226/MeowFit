/** 颜色工具：把用户选定的主题色换算成一整套界面用色。
 *
 * 规范第七节允许个性化配色；界面既要保证文字可读，也要让派生色（悬停、浅底、
 * 反白文字）与主色协调，因此由主色统一推导，而不是让用户一项项去调。
 */

export interface Rgb {
  r: number;
  g: number;
  b: number;
}

export function parseHex(hex: string): Rgb | null {
  const value = hex.trim().replace(/^#/, "");
  const full =
    value.length === 3
      ? value
          .split("")
          .map((c) => c + c)
          .join("")
      : value;
  if (!/^[0-9a-fA-F]{6}$/.test(full)) return null;
  return {
    r: Number.parseInt(full.slice(0, 2), 16),
    g: Number.parseInt(full.slice(2, 4), 16),
    b: Number.parseInt(full.slice(4, 6), 16),
  };
}

export function toHex({ r, g, b }: Rgb): string {
  const clamp = (n: number) => Math.max(0, Math.min(255, Math.round(n)));
  return `#${[clamp(r), clamp(g), clamp(b)]
    .map((n) => n.toString(16).padStart(2, "0"))
    .join("")
    .toUpperCase()}`;
}

/** 按比例混合两色，`t = 0` 取 a，`t = 1` 取 b。 */
export function mix(a: Rgb, b: Rgb, t: number): Rgb {
  return {
    r: a.r + (b.r - a.r) * t,
    g: a.g + (b.g - a.g) * t,
    b: a.b + (b.b - a.b) * t,
  };
}

/** WCAG 相对亮度。 */
export function luminance({ r, g, b }: Rgb): number {
  const channel = (v: number) => {
    const s = v / 255;
    return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b);
}

/** WCAG 对比度，范围 1–21。 */
export function contrastRatio(a: Rgb, b: Rgb): number {
  const la = luminance(a);
  const lb = luminance(b);
  return (Math.max(la, lb) + 0.05) / (Math.min(la, lb) + 0.05);
}

const WHITE: Rgb = { r: 255, g: 255, b: 255 };
const BLACK: Rgb = { r: 0, g: 0, b: 0 };

/** 主色上应使用的文字色（取对比度更高的一侧）。 */
export function readableOn(color: Rgb): Rgb {
  return contrastRatio(color, WHITE) >= contrastRatio(color, BLACK) ? WHITE : BLACK;
}

export interface AccentPalette {
  accent: string;
  hover: string;
  soft: string;
  contrast: string;
}

/**
 * 由用户选定的主色推导出一整套界面用色。
 *
 * - 浅色模式下主色需与白色面有足够对比，过浅时自动压暗；
 * - 深色模式下主色需与深色面有足够对比，过暗时自动提亮；
 * - `soft` 是主色与界面底色混合出的浅底，用于选中态与强调块。
 */
export function accentPalette(input: string, scheme: "light" | "dark"): AccentPalette {
  const base = parseHex(input) ?? { r: 168, g: 118, b: 58 };
  const surface: Rgb = scheme === "light" ? { r: 255, g: 255, b: 255 } : { r: 31, g: 30, b: 35 };
  const background: Rgb = scheme === "light" ? { r: 247, g: 246, b: 243 } : { r: 23, g: 22, b: 26 };

  let accent = base;
  // 与界面底色的对比度至少 3:1，保证控件轮廓与强调文字可辨
  for (let step = 0; step < 20 && contrastRatio(accent, background) < 3; step += 1) {
    accent = scheme === "light" ? mix(accent, BLACK, 0.08) : mix(accent, WHITE, 0.08);
  }

  const hover = scheme === "light" ? mix(accent, BLACK, 0.1) : mix(accent, WHITE, 0.12);
  const soft = mix(accent, surface, scheme === "light" ? 0.88 : 0.82);

  return {
    accent: toHex(accent),
    hover: toHex(hover),
    soft: toHex(soft),
    contrast: toHex(readableOn(accent)),
  };
}
