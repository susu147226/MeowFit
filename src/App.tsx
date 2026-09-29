import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";

export default function App() {
  const [version, setVersion] = useState("");

  useEffect(() => {
    // 版本号取自 tauri.conf.json，避免在界面上再写一份需要同步的常量
    getVersion()
      .then(setVersion)
      .catch(() => setVersion(""));
  }, []);

  return (
    <div className="app">
      <header className="app-header">
        <h1 className="app-title">喵尺 MeowFit</h1>
        {version && <span className="app-version">v{version}</span>}
      </header>

      <main className="app-body">
        <p className="app-placeholder">
          P0 阶段：工程骨架已就位，功能尚未实现。
        </p>
      </main>

      <footer className="app-footer">
        <span>源码可见的专有软件 · 仅限个人非商业用途 · 禁止再分发</span>
        <span>© 2026 云舒眠眠</span>
      </footer>
    </div>
  );
}
