import { useStore } from "../store";
import { Button } from "./ui";

export default function AboutDialog() {
  const info = useStore((s) => s.info);
  const setAboutOpen = useStore((s) => s.setAboutOpen);

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-6"
      onClick={() => setAboutOpen(false)}
    >
      <div
        className="w-[460px] rounded-lg border border-border bg-surface p-5 shadow-xl"
        onClick={(e) => e.stopPropagation()}
      >
        <h2 className="text-[16px] font-semibold">喵尺 MeowFit</h2>
        <p className="mt-1 font-mono text-[11px] text-faint">{info ? `v${info.version}` : ""}</p>

        <dl className="mt-3 space-y-1 text-[12px]">
          <div className="flex gap-2">
            <dt className="w-20 shrink-0 text-faint">版权</dt>
            <dd>{info?.copyright ?? "© 2026 云舒眠眠"}</dd>
          </div>
          <div className="flex gap-2">
            <dt className="w-20 shrink-0 text-faint">许可证</dt>
            <dd>{info?.licenseName ?? "喵尺 MeowFit 许可证"}</dd>
          </div>
          <div className="flex gap-2">
            <dt className="w-20 shrink-0 text-faint">应用标识</dt>
            <dd className="font-mono text-[11px]">{info?.identifier}</dd>
          </div>
          <div className="flex gap-2">
            <dt className="w-20 shrink-0 text-faint">配置目录</dt>
            <dd className="break-all font-mono text-[11px]">{info?.configDir}</dd>
          </div>
        </dl>

        <p className="mt-3 text-[11px] leading-relaxed text-muted">
          本软件为<strong className="text-text">源码可见的专有软件</strong>，仅允许个人非商业用途，
          <strong className="text-text">禁止再分发</strong>。完整条款见仓库根目录的 LICENSE。
        </p>
        <p className="mt-2 text-[11px] leading-relaxed text-muted">
          随包分发的 FFmpeg 可执行文件是独立的第三方程序，适用 GPL 许可证，不受本项目私有许可证约束，
          可依 GPL 获得其完整源码。
        </p>
        <p className="mt-2 text-[11px] text-faint">
          完全本地处理：不上传素材、不收集遥测、不检查更新，程序不发起任何网络请求。
        </p>

        <div className="mt-4 flex justify-end">
          <Button onClick={() => setAboutOpen(false)}>关闭</Button>
        </div>
      </div>
    </div>
  );
}
