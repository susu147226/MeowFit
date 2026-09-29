import { useStore } from "../store";
import { ACCEL_LABEL, CODEC_LABEL, type VideoAccel, type VideoCodec } from "../types";
import { Checkbox, Field, Select, Tag } from "./ui";

const CODECS: VideoCodec[] = ["h264", "h265", "vp9", "av1"];
const ACCELS: VideoAccel[] = ["nvenc", "qsv", "amf"];

/** 硬件加速只对 H.264 / H.265 有实现；VP9 与 AV1 会自动回退软件编码（规范 6.9）。 */
function hardwareAvailable(codec: VideoCodec): boolean {
  return codec === "h264" || codec === "h265";
}

/**
 * 视频处理参数（规范 6.9）。
 *
 * 与图片处理一样是**全局**设置，写入 `settings.json` 的 `processing` 段。
 */
export default function VideoOptionsPanel() {
  const settings = useStore((s) => s.settings);
  const files = useStore((s) => s.files);
  const ffmpegCheck = useStore((s) => s.ffmpegCheck);
  const setProcessing = useStore((s) => s.setProcessing);

  if (!settings) return null;

  const { processing } = settings;
  const codec = processing.videoEncoder as VideoCodec;
  const videos = files.filter((f) => f.kind === "video" && f.video);
  const hdrCount = videos.filter((f) => f.video?.isHdr).length;
  const canAccelerate = hardwareAvailable(codec);

  return (
    <section className="space-y-3 border-t border-border pt-3">
      <div className="flex items-center gap-2">
        <h3 className="text-[11px] font-semibold text-muted">视频处理</h3>
        <Tag>全局</Tag>
        {ffmpegCheck &&
          (ffmpegCheck.ffmpegFound ? (
            <Tag tone="ok">FFmpeg 就绪</Tag>
          ) : (
            <Tag tone="danger">未找到 FFmpeg</Tag>
          ))}
      </div>

      {ffmpegCheck && !ffmpegCheck.ffmpegFound && (
        <div className="rounded-md border border-danger/40 bg-danger-soft px-2.5 py-2 text-[11px] leading-relaxed text-danger">
          视频与动图需要 FFmpeg。请把 GPL 构建的 <span className="font-mono">ffmpeg.exe</span> 与{" "}
          <span className="font-mono">ffprobe.exe</span> 放到{" "}
          <span className="font-mono">src-tauri/resources/ffmpeg/win-x64/</span>。
        </div>
      )}

      {ffmpegCheck?.ffmpegFound && ffmpegCheck.missingEncoders.length > 0 && (
        <div className="rounded-md border border-warn/40 bg-warn-soft px-2.5 py-2 text-[11px] leading-relaxed text-warn">
          当前 FFmpeg 构建缺少：{ffmpegCheck.missingEncoders.join("、")}
          {ffmpegCheck.missingHevcDecoder ? "、HEVC 解码器" : ""}。对应功能无法使用。
        </div>
      )}

      <Field label="编码器" hint="四种编码器全部提供">
        <Select
          value={codec}
          onChange={(value) => setProcessing({ videoEncoder: value as VideoCodec })}
          options={CODECS.map((c) => ({ value: c, label: CODEC_LABEL[c] }))}
        />
      </Field>

      <Field label={`质量 CRF ${processing.videoCrf}`} hint="数值越小画质越好、体积越大">
        <input
          type="range"
          min={0}
          max={51}
          value={processing.videoCrf}
          onChange={(e) => setProcessing({ videoCrf: Number(e.target.value) })}
          className="w-full accent-accent"
        />
      </Field>

      <Checkbox
        checked={processing.hardwareAccel && canAccelerate}
        onChange={(enabled) => setProcessing({ hardwareAccel: enabled })}
      >
        硬件加速编码
      </Checkbox>

      {processing.hardwareAccel && canAccelerate && (
        <Field label="加速实现" hint="编码失败时会自动回退软件编码，并在日志中记录">
          <Select
            value={processing.videoAccel}
            onChange={(value) => setProcessing({ videoAccel: value as VideoAccel })}
            options={ACCELS.map((a) => ({ value: a, label: ACCEL_LABEL[a] }))}
          />
        </Field>
      )}

      {!canAccelerate && (
        <p className="text-[11px] text-faint">
          {codec.toUpperCase()} 没有对应的硬件编码实现，将使用软件编码。
        </p>
      )}

      <Checkbox
        checked={processing.hdrTonemapToSdr}
        onChange={(tonemap) => setProcessing({ hdrTonemapToSdr: tonemap })}
      >
        色调映射到 SDR
      </Checkbox>
      <p className="text-[11px] leading-relaxed text-faint">
        {processing.hdrTonemapToSdr
          ? "已开启：HDR 画面会被转换为 SDR。转换可能带来偏色，亮部与暗部细节也会与原片不同。"
          : "默认保持 HDR 原样传递，不加任何色彩滤镜。"}
        {hdrCount > 0 && ` 本次素材中有 ${hdrCount} 个 HDR 视频。`}
      </p>

      <p className="text-[11px] leading-relaxed text-faint">
        音频默认直接复制不重新编码；时长、帧率、字幕流与章节一并保留。
      </p>
    </section>
  );
}
