import { useStore } from "../store";
import { Button } from "./ui";

export default function TopBar() {
  const info = useStore((s) => s.info);
  const setAboutOpen = useStore((s) => s.setAboutOpen);
  const setAppearanceOpen = useStore((s) => s.setAppearanceOpen);

  return (
    <header className="flex shrink-0 items-center gap-3 border-b border-border bg-surface px-4 py-2.5">
      {/* 占位 Logo：最终图标由作者提供后替换（规范 7.1） */}
      <span
        aria-hidden
        className="flex h-7 w-7 items-center justify-center rounded-md border border-border bg-accent-soft text-[13px] font-semibold text-accent"
      >
        M
      </span>

      <h1 className="text-[15px] font-semibold tracking-wide whitespace-nowrap">喵尺 MeowFit</h1>
      <span className="rounded bg-surface-3 px-1.5 py-0.5 font-mono text-[11px] text-faint">
        {info ? `v${info.version}` : "…"}
      </span>

      <span className="ml-auto flex items-center gap-2">
        {info && !info.configPersistent && (
          <span className="tag bg-warn-soft text-warn" title={`配置目录：${info.configDir}`}>
            配置未能持久化
          </span>
        )}
        <Button onClick={() => setAppearanceOpen(true)} title="外观、配色与布局">
          外观
        </Button>
        <Button onClick={() => setAboutOpen(true)}>关于</Button>
      </span>
    </header>
  );
}
