import { useStore } from "../store";
import {
  FORMAT_LABEL,
  RESAMPLE_LABEL,
  type OutputFormat,
  type Resample,
  type SvgSizeMode,
} from "../types";
import { Checkbox, Field, Select, Tag, TextInput } from "./ui";

const RESAMPLES: Resample[] = ["lanczos3", "bicubic", "bilinear", "nearest"];
const FORMATS: OutputFormat[] = ["keep", "png", "jpeg", "webp"];
const SVG_MODES: SvgSizeMode[] = ["pixel", "dpi"];

/**
 * 图片处理参数（规范 6.8 / 10.5）。
 *
 * 这些是**全局**设置（写入 `settings.json`），与上方按整体 / 分组 / 单文件分层的
 * 缩放参数不是一回事，因此单独成块。
 */
export default function ImageOptionsPanel() {
  const settings = useStore((s) => s.settings);
  const files = useStore((s) => s.files);
  const setProcessing = useStore((s) => s.setProcessing);
  const setOutput = useStore((s) => s.setOutput);

  if (!settings) return null;

  const { processing, output } = settings;
  const hasSvg = files.some((f) => f.kind === "svg");
  const svgUndeclared = files.some((f) => f.kind === "svg" && !f.svgDeclared);
  const converting = output.outputFormat !== "keep";

  return (
    <section className="space-y-3 border-t border-border pt-3">
      <div className="flex items-center gap-2">
        <h3 className="text-[11px] font-semibold text-muted">图片处理</h3>
        <Tag>全局</Tag>
      </div>

      <Field label="重采样算法">
        <Select
          value={processing.resample}
          onChange={(value) => setProcessing({ resample: value as Resample })}
          options={RESAMPLES.map((r) => ({ value: r, label: RESAMPLE_LABEL[r] }))}
        />
      </Field>

      <Field
        label={`质量 ${processing.jpgQuality}`}
        hint="仅对 JPEG 生效；WebP 目前为无损编码，不适用质量参数"
      >
        <input
          type="range"
          min={1}
          max={100}
          value={processing.jpgQuality}
          onChange={(e) => setProcessing({ jpgQuality: Number(e.target.value) })}
          className="w-full accent-accent"
        />
      </Field>

      <Field label="输出格式" hint={converting ? "转换后扩展名会随之改变" : "保持原格式输出"}>
        <Select
          value={output.outputFormat}
          onChange={(value) => setOutput({ outputFormat: value as OutputFormat })}
          options={FORMATS.map((f) => ({ value: f, label: FORMAT_LABEL[f] }))}
        />
      </Field>

      <Checkbox
        checked={!output.stripRedundantMetadata}
        onChange={(keepAll) => setOutput({ stripRedundantMetadata: !keepAll })}
      >
        保留全部元数据（含缩略图等冗余数据）
      </Checkbox>
      <p className="text-[11px] leading-relaxed text-faint">
        关闭时保留 ICC 色彩配置与基础 EXIF / XMP，剥离其余冗余数据以减小体积。
      </p>

      <Field label="不支持透明时的填充色" hint="仅用于输出 JPEG 等无透明通道的格式">
        <input
          type="color"
          value={output.backgroundFillColor}
          onChange={(e) => setOutput({ backgroundFillColor: e.target.value.toUpperCase() })}
          className="h-7 w-full cursor-pointer rounded-md border border-border bg-surface"
        />
      </Field>

      {/* 未声明尺寸的 SVG 必须由用户选择基准方式（规范 10.5） */}
      {(hasSvg || svgUndeclared) && (
        <div className="space-y-2 rounded-md border border-border bg-surface-2 p-2.5">
          <div className="flex items-center gap-2">
            <span className="text-[11px] font-semibold text-muted">SVG 基准尺寸</span>
            {!svgUndeclared && <Tag>本次素材均已声明尺寸</Tag>}
          </div>
          <Select
            value={processing.svgSizeMode}
            onChange={(value) => setProcessing({ svgSizeMode: value as SvgSizeMode })}
            options={SVG_MODES.map((m) => ({
              value: m,
              label: m === "pixel" ? "直接填写目标像素尺寸（默认）" : "按 DPI 换算",
            }))}
          />
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
          <p className="text-[11px] leading-relaxed text-faint">
            仅在 SVG 未声明 width / height 时生效；已声明尺寸的 SVG 始终以其声明值为基准。
          </p>
        </div>
      )}
    </section>
  );
}
