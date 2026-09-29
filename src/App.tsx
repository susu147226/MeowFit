import { Fragment, useCallback, useEffect, useRef, useState } from "react";
import type { PointerEvent as ReactPointerEvent } from "react";

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
import { PanelDragContext } from "./components/ui";
import * as layoutLib from "./lib/layout";
import { type Layout, type PanelId } from "./lib/layout";
import { useStore } from "./store";

/** 拖拽分区时的落点。 */
type DropTarget =
  | { kind: "column"; columnId: string; index: number }
  | { kind: "new-column"; atIndex: number };

const EDGE_ZONE = 26;

/** 可拖拽的分隔条：`vertical` 拖动改宽度，`horizontal` 拖动改高度。 */
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
      title="拖动以调整大小"
      onMouseDown={() => setDragging(true)}
      className={`${base} shrink-0 transition ${dragging ? "bg-accent" : "bg-border hover:bg-accent"}`}
    />
  );
}

function PanelById({ id }: { id: PanelId }) {
  switch (id) {
    case "files":
      return <FileList className="min-h-0 flex-1" />;
    case "settings":
      return <SettingsPanel className="min-h-0 flex-1" />;
    case "preview":
      return <PreviewPanel className="min-h-0 flex-1" />;
    case "run":
      return <RunPanel className="min-h-0 flex-1" />;
  }
}

export default function App() {
  const init = useStore((s) => s.init);
  const aboutOpen = useStore((s) => s.aboutOpen);
  const appearanceOpen = useStore((s) => s.appearanceOpen);
  const backgroundUrl = useStore((s) => s.backgroundUrl);
  const theme = useStore((s) => s.settings?.theme);
  const layout = useStore((s) => s.layout);
  const applyLayout = useStore((s) => s.applyLayout);

  // 拖动中先改本地副本，松手才落盘，避免每次移动都写一次配置
  const [draft, setDraft] = useState<Layout | null>(null);
  const [dragPanel, setDragPanel] = useState<PanelId | null>(null);
  const [dropTarget, setDropTarget] = useState<DropTarget | null>(null);
  const workAreaRef = useRef<HTMLDivElement>(null);

  const shown = draft ?? layout;

  useEffect(() => {
    void init();
  }, [init]);

  // 外观默认跟随系统：系统配色变化时实时切换（仅在外观模式为「跟随系统」时生效）。
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

  useEffect(() => {
    document.documentElement.dataset.bg = backgroundUrl ? "on" : "off";
  }, [backgroundUrl]);

  /** 根据指针位置算出落点：列内的插入位置，或最左／最右的新建列。 */
  const computeTarget = useCallback(
    (x: number, y: number): DropTarget | null => {
      const area = workAreaRef.current;
      if (!area) return null;
      const areaRect = area.getBoundingClientRect();
      if (x < areaRect.left || x > areaRect.right) return null;

      const columnEls = [...area.querySelectorAll<HTMLElement>("[data-column-id]")];
      if (columnEls.length === 0) return null;

      // 最左／最右的窄条 = 新建一列
      const firstRect = columnEls[0].getBoundingClientRect();
      const lastRect = columnEls[columnEls.length - 1].getBoundingClientRect();
      if (x < firstRect.left + EDGE_ZONE) return { kind: "new-column", atIndex: 0 };
      if (x > lastRect.right - EDGE_ZONE) return { kind: "new-column", atIndex: columnEls.length };

      for (let i = 0; i < columnEls.length; i += 1) {
        const rect = columnEls[i].getBoundingClientRect();
        if (x < rect.left || x > rect.right) continue;
        const columnId = columnEls[i].dataset.columnId!;
        const panelEls = [...columnEls[i].querySelectorAll<HTMLElement>("[data-panel-id]")];
        let index = panelEls.length;
        for (let k = 0; k < panelEls.length; k += 1) {
          const panelRect = panelEls[k].getBoundingClientRect();
          if (y < panelRect.top + panelRect.height / 2) {
            index = k;
            break;
          }
        }
        return { kind: "column", columnId, index };
      }
      return null;
    },
    [],
  );

  const startDrag = useCallback(
    (panelId: string, event: ReactPointerEvent) => {
      event.preventDefault();
      setDragPanel(panelId as PanelId);
      setDropTarget(null);

      const move = (e: PointerEvent) => setDropTarget(computeTarget(e.clientX, e.clientY));
      const finish = (e: PointerEvent) => {
        window.removeEventListener("pointermove", move);
        window.removeEventListener("pointerup", finish);
        window.removeEventListener("keydown", onKey);
        const target = computeTarget(e.clientX, e.clientY);
        setDragPanel(null);
        setDropTarget(null);
        if (!target) return;

        const current = useStore.getState().layout;
        const next =
          target.kind === "column"
            ? layoutLib.movePanel(current, panelId as PanelId, target.columnId, target.index)
            : layoutLib.movePanelToNewColumn(current, panelId as PanelId, target.atIndex);
        applyLayout(next);
      };
      const onKey = (e: KeyboardEvent) => {
        if (e.key !== "Escape") return;
        window.removeEventListener("pointermove", move);
        window.removeEventListener("pointerup", finish);
        window.removeEventListener("keydown", onKey);
        setDragPanel(null);
        setDropTarget(null);
      };

      window.addEventListener("pointermove", move);
      window.addEventListener("pointerup", finish);
      window.addEventListener("keydown", onKey);
    },
    [applyLayout, computeTarget],
  );

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

        {/* 主工作区：列与列内分区都由用户拖动排布 */}
        <PanelDragContext.Provider value={{ start: startDrag, draggingId: dragPanel }}>
          <div ref={workAreaRef} className="relative flex min-h-0 flex-1 bg-surface">
            {/* 新建列的落点提示 */}
            {dragPanel && dropTarget?.kind === "new-column" && (
              <div
                className="pointer-events-none absolute inset-y-0 z-20 w-[3px] bg-accent"
                style={{
                  left: dropTarget.atIndex === 0 ? 0 : undefined,
                  right: dropTarget.atIndex === 0 ? undefined : 0,
                }}
              />
            )}

            {shown.columns.map((column, columnIndex) => (
              <div key={column.id} className="flex min-h-0 min-w-0">
                {columnIndex > 0 && (
                  <Splitter
                    orientation="vertical"
                    onDrag={(delta) => {
                      const left = shown.columns[columnIndex - 1];
                      setDraft(
                        // 左边的列若可调就调左边，否则调右边——保证「向右拖＝左列变宽」
                        left.width !== null
                          ? layoutLib.setColumnWidth(shown, left.id, left.width + delta)
                          : layoutLib.setColumnWidth(
                              shown,
                              column.id,
                              (column.width ?? 350) - delta,
                            ),
                      );
                    }}
                    onCommit={() => {
                      if (draft) applyLayout(draft);
                      setDraft(null);
                    }}
                  />
                )}

                <div
                  data-column-id={column.id}
                  className={`flex min-h-0 min-w-0 flex-col ${column.width === null ? "flex-1" : "shrink-0"}`}
                  style={column.width === null ? undefined : { width: column.width }}
                >
                  {column.panels.map((panelId, index) => (
                    <Fragment key={panelId}>
                      {index > 0 && (
                        <Splitter
                          orientation="horizontal"
                          onDrag={(delta) =>
                            setDraft(
                              layoutLib.setColumnTailHeight(shown, column.id, column.tailHeight - delta),
                            )
                          }
                          onCommit={() => {
                            if (draft) applyLayout(draft);
                            setDraft(null);
                          }}
                        />
                      )}

                      {dragPanel &&
                        dropTarget?.kind === "column" &&
                        dropTarget.columnId === column.id &&
                        dropTarget.index === index && <div className="h-[2px] shrink-0 bg-accent" />}

                      <div
                        className="flex min-h-0 flex-col overflow-hidden"
                        style={index === 0 ? { flex: 1 } : { height: column.tailHeight }}
                      >
                        <PanelById id={panelId} />
                      </div>
                    </Fragment>
                  ))}

                  {dragPanel &&
                    dropTarget?.kind === "column" &&
                    dropTarget.columnId === column.id &&
                    dropTarget.index === column.panels.length && (
                      <div className="h-[2px] shrink-0 bg-accent" />
                    )}
                </div>
              </div>
            ))}
          </div>
        </PanelDragContext.Provider>

        <Splitter
          orientation="horizontal"
          onDrag={(delta) =>
            // 向上拖 = 日志变高
            setDraft(layoutLib.setLogHeight(shown, shown.logHeight - delta))
          }
          onCommit={() => {
            if (draft) applyLayout(draft);
            setDraft(null);
          }}
        />
        <LogPanel height={shown.logHeight} />
      </div>

      {aboutOpen && <AboutDialog />}
      {appearanceOpen && <AppearanceDialog />}
      <GroupingConfirm />
    </>
  );
}
