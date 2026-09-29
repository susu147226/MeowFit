import { useStore } from "../store";
import { Button } from "./ui";

export default function TopBar() {
  const info = useStore((s) => s.info);
  const theme = useStore((s) => s.theme);
  const toggleTheme = useStore((s) => s.toggleTheme);
  const setAboutOpen = useStore((s) => s.setAboutOpen);

  return (
    <header className="flex shrink-0 items-center gap-3 border-b border-border bg-surface px-4 py-2.5">
      <h1 className="text-[15px] font-semibold tracking-wide">喵尺 MeowFit</h1>
      <span className="font-mono text-[11px] text-faint">{info ? `v${info.version}` : ""}</span>
      <span className="text-[11px] text-faint">仅本机处理 · 不联网</span>

      <span className="ml-auto flex items-center gap-2">
        {info && !info.configPersistent && (
          <span
            className="rounded border border-warn/40 bg-warn-soft px-2 py-0.5 text-[11px] text-warn"
            title={`配置目录：${info.configDir}`}
          >
            配置未能持久化
          </span>
        )}
        <Button onClick={toggleTheme} title="切换浅色 / 深色主题">
          {theme === "dark" ? "深色" : "浅色"}
        </Button>
        <Button onClick={() => setAboutOpen(true)}>关于</Button>
      </span>
    </header>
  );
}
