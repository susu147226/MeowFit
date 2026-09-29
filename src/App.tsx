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

      <div className="flex min-h-0 flex-1">
        {/* 素材列表区 */}
        <FileList />

        {/* 参数设置区 / 预览区 / 执行与进度区 */}
        <div className="flex w-[470px] shrink-0 flex-col overflow-hidden border-l border-border">
          <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
            <SettingsPanel />
          </div>
          <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
            <PreviewPanel />
          </div>
          <div className="flex max-h-[38%] min-h-0 flex-col overflow-hidden">
            <RunPanel />
          </div>
        </div>
      </div>

      {/* 日志区 */}
      <LogPanel />

      {aboutOpen && <AboutDialog />}
      <GroupingConfirm />
    </div>
  );
}
