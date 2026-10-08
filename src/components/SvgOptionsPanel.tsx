import { useStore } from "../store";
import { type SvgSizeMode } from "../types";
import { Field, Select, Tag, TextInput } from "./ui";

const SVG_MODES: SvgSizeMode[] = ["pixel", "dpi"];

/**
 * SVG 专属处理参数（规范 10.5）。从「图片处理」里拆出来，
 * 只在当前作用域涉及 SVG 时显示，避免对纯位图用户造成冗余。
 */
export default function SvgOptionsPanel() {
  const settings = useStore((s) => s.settings);
  const setProcessing = useStore((s) => s.setProcessing);

  if (!settings) return null;

  const { processing } = settings;

  return (
    <section className="space-y-3 border-t border-border pt-3">
      <div className="flex items-center gap-2">
        <h3 className="text-[11px] font-semibold text-muted">SVG 处理</h3>
        <Tag>全局</Tag>
      </div>

      <Field
        label="未声明尺寸时的基准方式"
        hint="仅在 SVG 未声明 width / height 时生效；已声明尺寸的 SVG 始终以其声明值为基准"
      >
        <Select
          value={processing.svgSizeMode}
          onChange={(value) => setProcessing({ svgSizeMode: value as SvgSizeMode })}
          options={SVG_MODES.map((m) => ({
            value: m,
            label: m === "pixel" ? "直接填写目标像素尺寸（默认）" : "按 DPI 换算",
          }))}
        />
      </Field>

      {processing.svgSizeMode === "dpi" && (
        <Field label="DPI" hint="基准像素 = viewBox 宽高 × DPI ÷ 96">
          <TextInput
            value={String(processing.svgDpi)}
            onChange={(text) => {
              const value = Number(text);
              if (Number.isFinite(value) && value > 0) setProcessing({ svgDpi: value });
            }}
            placeholder="96"
          />
        </Field>
      )}
    </section>
  );
}
