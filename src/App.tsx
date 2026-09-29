import { useEffect, useRef, useState } from "react";

import AboutDialog from "./components/AboutDialog";
import AppearanceDialog from "./components/AppearanceDialog";
import FileList from "./components/FileList";
import GroupingConfirm from "./components/GroupingConfirm";
import LogPanel from "./components/LogPanel";
import PreviewPanel from "./components/PreviewPanel";
import RunPanel from "./components/RunPanel";
import ScanBar from "./components/ScanBar";
import SettingsPanel from "./components/SettingsPanel";
import TopBar from "./components/TopBar";
import { useStore } from "./store";

const MIN_COLUMN = 260;
const MAX_COLUMN = 560;

/** 可拖拽的分隔条：用于个性化调整各列宽度（规范第七节）。 */
function Divider({ onDrag, onCommit }: { onDrag: (delta: number) => void; onCommit: () => void }) {
  const [dragging, setDragging] = useState(false);
  const dragRef = useRef(onDrag);
  const commitRef = useRef(onCommit);
  dragRef.current = onDrag;
  commitRef.current = onCommit;

  useEffect(() => {
    if (!dragging) return;
    const move = (event: MouseEvent) => dragRef.current(event.movementX);
    const up = () => {
      setDragging(false);
      commitRef.current();
    };
    window.addEventListener("mousemove", move);
    window.addEventListener("mouseup", up);
    return () => {
      window.removeEventListener("mousemove", move);
      window.removeEventListener("mouseup", up);
    };
  }, [dragging]);

  return (
    <div
      role="separator"
      aria-orientation="vertical"
      onMouseDown={() => setDragging(true)}
      className={`w-[3px] shrink-0 cursor-col-resize transition ${
        dragging ? "bg-accent" : "bg-border hover:bg-accent"
      }`}
    />
  );
}

export default function App() {
  const init = useStore((s) => s.init);
  const aboutOpen = useStore((s) => s.aboutOpen);
  const appearanceOpen = useStore((s) => s.appearanceOpen);
  const backgroundUrl = useStore((s) => s.backgroundUrl);
  const theme = useStore((s) => s.settings?.theme);
  const setTheme = useStore((s) => s.setTheme);

  // 拖拽期间用本地值实时跟手，松手后再落盘，避免每移动一像素就写一次配置
  const [dragSidebar, setDragSidebar] = useState<number | null>(null);
  const [dragPreview, setDragPreview] = useState<number | null>(null);

  const sidebarWidth = dragSidebar ?? theme?.sidebarWidth ?? 350;
  const previewWidth = dragPreview ?? theme?.previewWidth ?? 350;

  useEffect(() => {
    void init();
  }, [init]);

  // 外观默认跟随系统：系统配色变化时实时切换（仅在外观模式为「跟随系统」时生效）。
  // 放在外壳而不是初始化流程里，避免初始化失败时连带失去跟随能力。
  useEffect(() => {
    const query = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = () => {
      if (useStore.getState().settings?.theme.mode === "system") {
        void useStore.getState().applyAppearance();
      }
    };
    query.addEventListener("change", onChange);
    return () => query.removeEventListener("change", onChange);
  }, []);

  // 背景图生效时把面板转为半透明，让背景透出来（规范 6.14）
  useEffect(() => {
    document.documentElement.dataset.bg = backgroundUrl ? "on" : "off";
  }, [backgroundUrl]);

  const clamp = (value: number) => Math.max(MIN_COLUMN, Math.min(MAX_COLUMN, value));

  return (
    <>
      {backgroundUrl && (
        <>
          <div
            className="app-backdrop"
            style={{
              backgroundImage: `url(${backgroundUrl})`,
              opacity: (theme?.backgroundOpacity ?? 100) / 100,
            }}
          />
          {theme?.autoScrim && <div className="app-scrim" />}
        </>
      )}

      <div className="app-content flex h-full flex-col bg-bg text-text">
        <TopBar />
        <ScanBar />

        {/* 主工作区：素材列表区 ｜ 参数设置区 ｜ 预览区 + 执行与进度区（规范第七节）；
            列宽可由分隔条拖动，属个性化布局设置 */}
        <div className="flex min-h-0 flex-1 bg-surface">
          <FileList className="min-w-[260px] flex-1" />

          <Divider
            onDrag={(delta) => setDragSidebar((v) => clamp((v ?? sidebarWidth) + delta))}
            onCommit={() => {
              if (dragSidebar !== null) void setTheme({ sidebarWidth: dragSidebar });
              setDragSidebar(null);
            }}
          />

          <SettingsPanel className="shrink-0 border-l border-border" width={sidebarWidth} />

          <Divider
            onDrag={(delta) => setDragPreview((v) => clamp((v ?? previewWidth) + delta))}
            onCommit={() => {
              if (dragPreview !== null) void setTheme({ previewWidth: dragPreview });
              setDragPreview(null);
            }}
          />

          <div className="flex shrink-0 flex-col border-l border-border" style={{ width: previewWidth }}>
            <PreviewPanel className="min-h-0 flex-1" />
            <RunPanel className="max-h-[46%] shrink-0 border-t border-border" />
          </div>
        </div>

        <LogPanel />
      </div>

      {aboutOpen && <AboutDialog />}
      {appearanceOpen && <AppearanceDialog />}
      <GroupingConfirm />
    </>
  );
}
