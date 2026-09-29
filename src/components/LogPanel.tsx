import { useEffect, useRef, useState } from "react";

import { useStore } from "../store";
import { Button } from "./ui";

const LEVEL_CLASS = {
  INFO: "text-muted",
  WARN: "text-warn",
  ERROR: "text-danger",
} as const;

export default function LogPanel({ height }: { height?: number }) {
  const logs = useStore((s) => s.logs);
  const [open, setOpen] = useState(false);
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (open) bottomRef.current?.scrollIntoView({ block: "end" });
  }, [logs.length, open]);

  const hasError = logs.some((entry) => entry.level === "ERROR");

  return (
    <section className="flex shrink-0 flex-col border-t border-border bg-surface">
      <div className="flex shrink-0 items-center gap-2 border-b border-border bg-surface-2 px-3.5 py-1.5">
        <button
          type="button"
          onClick={() => setOpen((v) => !v)}
          className="flex flex-1 cursor-pointer items-center gap-2 text-left transition hover:text-text"
        >
          <span className={`text-[10px] text-faint transition ${open ? "rotate-90" : ""}`}>▶</span>
          <h2 className="panel-title">日志区</h2>
          <span className="text-[11px] text-faint">{logs.length} 条</span>
          {hasError && !open && <span className="tag bg-danger-soft text-danger">有错误</span>}
        </button>
        <Button variant="ghost" onClick={() => useStore.setState({ logs: [] })}>
          清空
        </Button>
      </div>

      {open && (
        <div
          className="min-h-0 overflow-auto px-3.5 py-2 font-mono text-[11px] leading-relaxed"
          style={{ height: height ?? 190 }}
        >
          {logs.length === 0 ? (
            <p className="py-2 text-faint">暂无日志</p>
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
      )}
    </section>
  );
}
