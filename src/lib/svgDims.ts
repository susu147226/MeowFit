import type { ScannedFile, Settings } from "../types";

/**
 * 参与尺寸计算的基准尺寸（规范 10.5）。
 *
 * 未声明 `width` / `height` 的 SVG 由用户选择基准方式：
 * - `pixel`（默认）：直接以 viewBox 尺寸为基准，用户自行填写目标像素尺寸；
 * - `dpi`：基准 = viewBox 宽高 × DPI ÷ 96，默认 DPI 96 时即 1:1。
 *
 * 已声明尺寸的 SVG 始终以其声明值为基准，不受该选项影响。
 */
export function dimsOf(
  file: Pick<ScannedFile, "width" | "height" | "kind" | "svgDeclared">,
  settings: Pick<Settings, "processing"> | null,
): { width: number; height: number } {
  const width = file.width ?? 0;
  const height = file.height ?? 0;

  if (file.kind === "svg" && !file.svgDeclared && settings?.processing.svgSizeMode === "dpi") {
    const scale = (settings.processing.svgDpi || 96) / 96;
    return { width: Math.round(width * scale), height: Math.round(height * scale) };
  }
  return { width, height };
}
