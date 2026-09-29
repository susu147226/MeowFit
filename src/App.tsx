import { useEffect } from "react";

import AboutDialog from "./components/AboutDialog";
import FileList from "./components/FileList";
import GroupingConfirm from "./components/GroupingConfirm";
import LogPanel from "./components/LogPanel";
import PreviewPanel from "./components/PreviewPanel";
import RunPanel from "./components/RunPanel";
import ScanBar from "./components/ScanBar";
import SettingsPanel from "./components/SettingsPanel";
import TopBar from "./components/TopBar";
import { useStore } from "./store";

export default function App() {
  const init = useStore((s) => s.init);
  const aboutOpen = useStore((s) => s.aboutOpen);

  useEffect(() => {
    void init();
  }, [init]);

  return (
    <div className="flex h-full flex-col bg-bg text-text">
      <TopBar />
      <ScanBar />

      {/* 主工作区：素材列表区 ｜ 参数设置区 ｜ 预览区 + 执行与进度区（规范第七节） */}
      <div className="flex min-h-0 flex-1 bg-surface">
        <FileList className="min-w-[260px] flex-1" />
        <SettingsPanel className="w-[350px] shrink-0 border-l border-border" />
        <div className="flex w-[350px] shrink-0 flex-col border-l border-border">
          <PreviewPanel className="min-h-0 flex-1" />
          <RunPanel className="max-h-[46%] shrink-0 border-t border-border" />
        </div>
      </div>

      <LogPanel />

      {aboutOpen && <AboutDialog />}
      <GroupingConfirm />
    </div>
  );
}
