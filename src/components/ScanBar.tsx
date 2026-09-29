import { useEffect, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";

import { pickFolder } from "../api";
import { useStore } from "../store";
import { Button, TextInput } from "./ui";

export default function ScanBar() {
  const root = useStore((s) => s.root);
  const scanning = useStore((s) => s.scanning);
  const scanOptions = useStore((s) => s.scanOptions);
  const setScanOptions = useStore((s) => s.setScanOptions);
  const scanFolder = useStore((s) => s.scanFolder);
  const settings = useStore((s) => s.settings);
  const log = useStore((s) => s.log);

  const [dragging, setDragging] = useState(false);
  const [pathInput, setPathInput] = useState("");
  const [includeText, setIncludeText] = useState(scanOptions.include.join(","));
  const [excludeText, setExcludeText] = useState(scanOptions.exclude.join(","));

  // 拖拽文件夹 / 拖拽多个文件（规范 6.1）。路径来自本机，不经过任何网络。
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;

    const handle = async (paths: string[]) => {
      if (paths.length === 0) return;
      const first = paths[0];
      if (/\.[^\\/]+$/.test(first)) {
        // 拖入多个文件时，以其所在文件夹为输入根目录
        const parent = first.replace(/[\\/][^\\/]*$/, "");
        log("INFO", `拖入 ${paths.length} 个文件，将以所在文件夹为输入：${parent}`);
        await scanFolder(parent);
      } else {
        await scanFolder(first);
      }
    };

    // 拖拽能力由 Tauri WebView 提供；不可用时退化为「不支持拖拽」，不影响其余功能
    try {
      void getCurrentWebview()
        .onDragDropEvent(async (event) => {
          if (event.payload.type === "over") {
            setDragging(true);
            return;
          }
          setDragging(false);
          if (event.payload.type !== "drop") return;
          await handle(event.payload.paths);
        })
        .then((fn) => {
          if (cancelled) fn();
          else unlisten = fn;
        })
        .catch(() => {
          log("WARN", "当前环境不支持拖拽载入，请改用「选择文件夹」按钮或粘贴路径");
        });
    } catch {
      log("WARN", "当前环境不支持拖拽载入，请改用「选择文件夹」按钮或粘贴路径");
    }

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [scanFolder, log]);

  const parseList = (text: string) =>
    text
      .split(",")
      .map((s) => s.trim())
      .filter(Boolean);

  const applyOptions = () => {
    setScanOptions({
      include: parseList(includeText),
      exclude: parseList(excludeText),
    });
    if (root) void scanFolder(root, { include: parseList(includeText), exclude: parseList(excludeText) });
  };

  return (
    <section
      className={`shrink-0 border-b bg-surface px-4 py-2 transition ${
        dragging ? "border-accent bg-accent-soft" : "border-border"
      }`}
    >
      <div className="flex flex-wrap items-center gap-2">
        <Button
          variant="primary"
          disabled={scanning}
          onClick={() => {
            void pickFolder().then((picked) => {
              if (picked) void scanFolder(picked);
            });
          }}
        >
          {scanning ? "扫描中…" : "选择文件夹"}
        </Button>

        <TextInput
          className="max-w-[280px]"
          value={pathInput}
          onChange={setPathInput}
          placeholder="粘贴路径后回车"
        />
        <Button
          disabled={!pathInput.trim() || scanning}
          onClick={() => void scanFolder(pathInput.trim())}
        >
          打开
        </Button>

        {settings && settings.recentFolders.length > 0 && (
          <select
            className="max-w-[260px] rounded border border-border bg-surface px-2 py-1 text-[12px] outline-none focus:border-accent"
            value=""
            onChange={(e) => {
              if (e.target.value) void scanFolder(e.target.value);
            }}
          >
            <option value="">最近使用的文件夹（{settings.recentFolders.length}）</option>
            {settings.recentFolders.map((folder) => (
              <option key={folder} value={folder}>
                {folder}
              </option>
            ))}
          </select>
        )}

        <label className="ml-auto flex items-center gap-1.5 text-[12px] text-muted">
          <input
            type="checkbox"
            checked={scanOptions.recursive}
            onChange={(e) => {
              setScanOptions({ recursive: e.target.checked });
              if (root) void scanFolder(root, { recursive: e.target.checked });
            }}
          />
          递归子文件夹
        </label>
        {!scanOptions.recursive && <span className="text-[11px] text-faint">（仅当前层）</span>}
      </div>

      <div className="mt-2 flex flex-wrap items-center gap-2 text-[12px] text-muted">
        <span className="text-[11px] text-faint">包含</span>
        <TextInput
          className="max-w-[200px]"
          value={includeText}
          onChange={setIncludeText}
          placeholder="如 icon_*,logo*"
        />
        <span className="text-[11px] text-faint">排除</span>
        <TextInput
          className="max-w-[200px]"
          value={excludeText}
          onChange={setExcludeText}
          placeholder="如 *_thumb.*,@2x"
        />
        <Button onClick={applyOptions} disabled={!root}>
          应用并重新扫描
        </Button>

        {root && (
          <span className="ml-auto max-w-[420px] truncate font-mono text-[11px] text-faint" title={root}>
            输入：{root}
          </span>
        )}
      </div>

      {dragging && (
        <p className="mt-1.5 text-[12px] text-accent">松开即可载入此文件夹</p>
      )}
    </section>
  );
}
