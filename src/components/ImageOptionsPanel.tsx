import { useStore } from "../store";
import {
  FORMAT_LABEL,
  RESAMPLE_LABEL,
  type OutputFormat,
  type Resample,
} from "../types";
import { Checkbox, Field, Select, Tag } from "./ui";

const RESAMPLES: Resample[] = ["lanczos3", "bicubic", "bilinear", "nearest"];
const FORMATS: OutputFormat[] = ["keep", "png", "jpeg", "webp"];

/**
 * 图片处理参数（规范 6.8）。SVG 专属项已拆到 SvgOptionsPanel。
 *
 * 这些是**全局**设置（写入 `settings.json`），与上方按整体 / 分组 / 单文件分层的
 * 缩放参数不是一回事，因此单独成块。
 */
export default function ImageOptionsPanel() {
  const settings = useStore((s) => s.settings);
  const setProcessing = useStore((s) => s.setProcessing);
  const setOutput = useStore((s) => s.setOutput);

  if (!settings) return null;

  const { processing, output } = settings;
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
    </section>
  );
}
