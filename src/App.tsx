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
import { DEFAULT_LAYOUT } from "./types";

const MIN_WIDTH = 240;
const MAX_WIDTH = 620;
const MIN_RUN_HEIGHT = 120;
const MAX_RUN_HEIGHT = 620;
const MIN_LOG_HEIGHT = 80;
const MAX_LOG_HEIGHT = 420;

/**
 * 可拖拽的分隔条（规范第七节允许个性化布局）。
 *
 * `vertical` 拖动改宽度，`horizontal` 拖动改高度；拖动期间只更新本地状态跟手，
 * 松手后才落盘，避免每移动一像素就写一次配置。
 */
function Splitter({
  orientation,
  onDrag,
  onCommit,
}: {
  orientation: "vertical" | "horizontal";
  onDrag: (delta: number) => void;
  onCommit: () => void;
}) {
  const [dragging, setDragging] = useState(false);
  const dragRef = useRef(onDrag);
  const commitRef = useRef(onCommit);
  dragRef.current = onDrag;
  commitRef.current = onCommit;

  useEffect(() => {
    if (!dragging) return;
    const move = (event: MouseEvent) =>
      dragRef.current(orientation === "vertical" ? event.movementX : event.movementY);
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
  }, [dragging, orientation]);

  const base = orientation === "vertical" ? "w-[3px] cursor-col-resize" : "h-[3px] cursor-row-resize";
  return (
    <div
      role="separator"
      aria-orientation={orientation}
      title="拖动以调整布局"
      onMouseDown={() => setDragging(true)}
      className={`${base} shrink-0 transition ${dragging ? "bg-accent" : "bg-border hover:bg-accent"}`}
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

  // 拖拽期间用本地值实时跟手，松手后再落盘
  const [dragSidebar, setDragSidebar] = useState<number | null>(null);
  const [dragPreview, setDragPreview] = useState<number | null>(null);
  const [dragRun, setDragRun] = useState<number | null>(null);
  const [dragLog, setDragLog] = useState<number | null>(null);

  const sidebarWidth = dragSidebar ?? theme?.sidebarWidth ?? DEFAULT_LAYOUT.sidebarWidth;
  const previewWidth = dragPreview ?? theme?.previewWidth ?? DEFAULT_LAYOUT.previewWidth;
  const runHeight = dragRun ?? theme?.runHeight ?? DEFAULT_LAYOUT.runHeight;
  const logHeight = dragLog ?? theme?.logHeight ?? DEFAULT_LAYOUT.logHeight;

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

  const clamp = (value: number, min: number, max: number) =>
    Math.max(min, Math.min(max, value));

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

        {/* 主工作区（规范第七节）；四条分隔条均可拖动，属个性化布局 */}
        <div className="flex min-h-0 flex-1 bg-surface">
          <FileList className="min-w-[240px] flex-1" />

          <Splitter
            orientation="vertical"
            onDrag={(d) => setDragSidebar((v) => clamp((v ?? sidebarWidth) + d, MIN_WIDTH, MAX_WIDTH))}
            onCommit={() => {
              if (dragSidebar !== null) void setTheme({ sidebarWidth: dragSidebar });
              setDragSidebar(null);
            }}
          />

          <SettingsPanel className="shrink-0" width={sidebarWidth} />

          <Splitter
            orientation="vertical"
            onDrag={(d) => setDragPreview((v) => clamp((v ?? previewWidth) + d, MIN_WIDTH, MAX_WIDTH))}
            onCommit={() => {
              if (dragPreview !== null) void setTheme({ previewWidth: dragPreview });
              setDragPreview(null);
            }}
          />

          <div className="flex shrink-0 flex-col" style={{ width: previewWidth }}>
            <PreviewPanel className="min-h-0 flex-1" />
            <Splitter
              orientation="horizontal"
              onDrag={(d) =>
                setDragRun((v) => clamp((v ?? runHeight) + d, MIN_RUN_HEIGHT, MAX_RUN_HEIGHT))
              }
              onCommit={() => {
                if (dragRun !== null) void setTheme({ runHeight: dragRun });
                setDragRun(null);
              }}
            />
            <RunPanel className="shrink-0" height={runHeight} />
          </div>
        </div>

        <Splitter
          orientation="horizontal"
          onDrag={(d) => {
            // 向上拖 = 日志变高，因此取反
            setDragLog((v) => clamp((v ?? logHeight) - d, MIN_LOG_HEIGHT, MAX_LOG_HEIGHT));
          }}
          onCommit={() => {
            if (dragLog !== null) void setTheme({ logHeight: dragLog });
            setDragLog(null);
          }}
        />
        <LogPanel height={logHeight} />
      </div>

      {aboutOpen && <AboutDialog />}
      {appearanceOpen && <AppearanceDialog />}
      <GroupingConfirm />
    </>
  );
}
