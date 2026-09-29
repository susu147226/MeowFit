import { useEffect, useRef } from "react";

import { useStore } from "../store";
import { Button } from "./ui";

const LEVEL_CLASS = {
  INFO: "text-muted",
  WARN: "text-warn",
  ERROR: "text-danger",
} as const;

export default function LogPanel() {
  const logs = useStore((s) => s.logs);
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ block: "end" });
  }, [logs.length]);

  return (
    <section className="flex h-[150px] shrink-0 flex-col border-t border-border bg-surface">
      <div className="flex shrink-0 items-center gap-2 border-b border-border bg-surface-2 px-3 py-1.5">
        <h2 className="text-[12px] font-semibold tracking-wide text-muted">日志区</h2>
        <span className="text-[11px] text-faint">{logs.length} 条</span>
        <span className="ml-auto flex items-center gap-2">
          <span className="text-[11px] text-faint">
            运行期日志面板；写入 logs/ 并按日期滚动、保留 30 天见 P5 阶段
          </span>
          <Button variant="ghost" onClick={() => useStore.setState({ logs: [] })}>
            清空
          </Button>
        </span>
      </div>

      <div className="min-h-0 flex-1 overflow-auto px-3 py-1 font-mono text-[11px] leading-relaxed">
        {logs.length === 0 ? (
          <p className="py-2 text-faint">暂无日志。</p>
        ) : (
          logs.map((entry, index) => (
            <div key={index} className="flex gap-2">
              <span className="shrink-0 text-faint">{entry.time}</span>
              <span className={`shrink-0 ${LEVEL_CLASS[entry.level]}`}>{entry.level}</span>
              <span className="whitespace-pre-wrap text-muted">{entry.message}</span>
            </div>
          ))
        )}
        <div ref={bottomRef} />
      </div>
    </section>
  );
}
