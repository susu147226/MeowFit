import { useEffect, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";

import { pickFolder, pickOutputFolder } from "../api";
import { useStore } from "../store";
import { Button, Checkbox, Select, TextInput } from "./ui";

const EMPTY_FOLDERS: readonly string[] = [];

export default function ScanBar() {
  const root = useStore((s) => s.root);
  const scanning = useStore((s) => s.scanning);
  const scanOptions = useStore((s) => s.scanOptions);
  const setScanOptions = useStore((s) => s.setScanOptions);
  const scanFolder = useStore((s) => s.scanFolder);
  // 选择器必须返回稳定引用：每次新建 [] 会让 zustand 判定状态变化并陷入无限重渲染
  const recentFolders = useStore((s) => s.settings?.recentFolders ?? EMPTY_FOLDERS);
  const log = useStore((s) => s.log);
  // 输出目录（规范 6.5 两种方式）——放在扫描栏，紧邻「递归子文件夹」
  const outputMode = useStore((s) => s.outputMode);
  const userOutputDir = useStore((s) => s.userOutputDir);
  const outputDir = useStore((s) => s.outputDir);
  const setOutputMode = useStore((s) => s.setOutputMode);
  const setUserOutputDir = useStore((s) => s.setUserOutputDir);

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
        log("INFO", `拖入 ${paths.length} 个文件，以所在文件夹为输入：${parent}`);
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
        .catch(() => log("WARN", "当前环境不支持拖拽载入"));
    } catch {
      log("WARN", "当前环境不支持拖拽载入");
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
    const include = parseList(includeText);
    const exclude = parseList(excludeText);
    setScanOptions({ include, exclude });
    if (root) void scanFolder(root, { include, exclude });
  };

  return (
    <section
      className={`shrink-0 border-b bg-surface px-4 py-2.5 transition ${
        dragging ? "border-accent bg-accent-soft" : "border-border"
      }`}
    >
      <div className="flex items-center gap-2">
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
          className="max-w-[260px]"
          value={pathInput}
          onChange={setPathInput}
          placeholder="或粘贴路径后回车"
        />
        <Button disabled={!pathInput.trim() || scanning} onClick={() => void scanFolder(pathInput.trim())}>
          打开
        </Button>

        {recentFolders.length > 0 && (
          <Select
            className="max-w-[240px]"
            title="最近使用的文件夹"
            value=""
            onChange={(folder) => {
              if (folder) void scanFolder(folder);
            }}
            options={[
              { value: "", label: `最近使用（${recentFolders.length}）` },
              ...recentFolders.map((folder) => ({ value: folder, label: folder })),
            ]}
          />
        )}

        <span className="ml-auto flex items-center gap-3">
          <Checkbox
            checked={scanOptions.recursive}
            onChange={(checked) => {
              setScanOptions({ recursive: checked });
              if (root) void scanFolder(root, { recursive: checked });
            }}
          >
            递归子文件夹
          </Checkbox>
        </span>
      </div>

      {/* 输出目录（规范 6.5 两种方式）：紧邻「递归子文件夹」 */}
      <div className="mt-2 flex items-center gap-2">
        <span className="shrink-0 text-[11px] text-faint">输出目录</span>
        <Select
          className="max-w-[210px]"
          value={outputMode}
          onChange={setOutputMode}
          options={[
            { value: "sibling", label: "跟随源文件夹（同级 output/）" },
            { value: "user", label: "每次任务由我指定" },
          ]}
        />
        {outputMode === "user" && (
          <>
            <Button
              onClick={() => {
                void pickOutputFolder().then((dir) => {
                  if (dir) setUserOutputDir(dir);
                });
              }}
            >
              选择目录
            </Button>
            <span className="min-w-0 truncate font-mono text-[11px] text-faint" title={userOutputDir ?? ""}>
              {userOutputDir ?? "尚未选择"}
            </span>
          </>
        )}
        {outputMode !== "user" && outputDir && (
          <span className="min-w-0 truncate font-mono text-[11px] text-faint" title={outputDir}>
            {outputDir}
          </span>
        )}
      </div>

      <div className="mt-2 flex items-center gap-2">
        <span className="shrink-0 text-[11px] text-faint">包含</span>
        <TextInput
          className="max-w-[180px]"
          value={includeText}
          onChange={setIncludeText}
          placeholder="icon_*,logo*"
        />
        <span className="shrink-0 text-[11px] text-faint">排除</span>
        <TextInput
          className="max-w-[180px]"
          value={excludeText}
          onChange={setExcludeText}
          placeholder="*_thumb.*,@2x"
        />
        <Button onClick={applyOptions} disabled={!root}>
          应用
        </Button>

        <span className="ml-auto min-w-0 truncate font-mono text-[11px] text-faint" title={root ?? ""}>
          {dragging ? "松开即可载入此文件夹" : (root ?? "尚未选择素材文件夹")}
        </span>
      </div>
    </section>
  );
}
