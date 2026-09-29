import { useStore } from "../store";
import { GIF_COLORS } from "../types";
import { Checkbox, Field, Select, Tag, TextInput } from "./ui";

/** 动图处理参数（规范 6.10）。颜色数与抖动是持久化设置，转码与目标体积是本次任务的选项。 */
export default function AnimationOptionsPanel() {
  const settings = useStore((s) => s.settings);
  const files = useStore((s) => s.files);
  const animationToVideo = useStore((s) => s.animationToVideo);
  const animationTargetBytes = useStore((s) => s.animationTargetBytes);
  const setProcessing = useStore((s) => s.setProcessing);
  const setAnimationToVideo = useStore((s) => s.setAnimationToVideo);
  const setAnimationTargetBytes = useStore((s) => s.setAnimationTargetBytes);

  const gifs = files.filter((f) => f.kind === "animated");
  if (!settings || gifs.length === 0) return null;

  const frames = gifs
    .map((f) => f.animation?.frames)
    .filter((n): n is number => typeof n === "number");
  const totalFrames = frames.reduce((sum, n) => sum + n, 0);

  return (
    <section className="space-y-3 border-t border-border pt-3">
      <div className="flex items-center gap-2">
        <h3 className="text-[11px] font-semibold text-muted">动图处理</h3>
        <Tag>{gifs.length} 个动图</Tag>
        {totalFrames > 0 && <Tag>共 {totalFrames} 帧</Tag>}
      </div>

      <Field label="调色板颜色数" hint="颜色数对体积影响显著：64 色通常只有 256 色的几分之一">
        <Select
          value={String(settings.processing.gifColors)}
          onChange={(value) => setProcessing({ gifColors: Number(value) })}
          options={GIF_COLORS.map((n) => ({ value: String(n), label: `${n} 色` }))}
        />
      </Field>

      <Checkbox
        checked={settings.processing.gifDither}
        onChange={(v) => setProcessing({ gifDither: v })}
      >
        启用抖动（渐变更平滑，但体积更大）
      </Checkbox>

      <Field label="输出格式" hint="转成视频通常能进一步大幅减小体积">
        <Select
          value={animationToVideo}
          onChange={setAnimationToVideo}
          options={[
            { value: "none", label: "保持动图格式" },
            { value: "mp4", label: "转为 MP4" },
            { value: "webm", label: "转为 WebM" },
          ]}
        />
      </Field>

      <Field
        label="目标体积上限（KB）"
        hint="留空表示不限制。填写后按「减色数 → 降尺寸 → 丢帧」逐级逼近，不会无限尝试"
      >
        <TextInput
          value={animationTargetBytes === null ? "" : String(Math.round(animationTargetBytes / 1024))}
          placeholder="例如 500"
          onChange={(text) => {
            const kb = Number(text.trim());
            setAnimationTargetBytes(text.trim() === "" || !Number.isFinite(kb) || kb <= 0 ? null : Math.round(kb * 1024));
          }}
        />
      </Field>

      <p className="text-[11px] leading-relaxed text-faint">
        帧数、每帧延迟、循环次数与透明通道都会保留；每帧等比缩放，保证帧间尺寸一致。
      </p>
    </section>
  );
}
