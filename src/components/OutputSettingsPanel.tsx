import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";

import { pickOutputFolder } from "../api";
import { useStore } from "../store";
import type { ProgressEvent } from "../types";
import { Button, Checkbox, Field, Select, Tag, TextInput } from "./ui";

/** 输出设置与执行选项（规范 6.5 / 6.7 / 6.13）。 */
export default function OutputSettingsPanel() {
  const settings = useStore((s) => s.settings);
  const outputMode = useStore((s) => s.outputMode);
  const userOutputDir = useStore((s) => s.userOutputDir);
  const dryRun = useStore((s) => s.dryRun);
  const backup = useStore((s) => s.backup);
  const skipUnchanged = useStore((s) => s.skipUnchanged);
  const verifyOutput = useStore((s) => s.verifyOutput);
  const outputDir = useStore((s) => s.outputDir);
  const progress = useStore((s) => s.progress);
  const running = useStore((s) => s.running);
  const diskFree = useStore((s) => s.diskFree);

  const setOutput = useStore((s) => s.setOutput);
  const setOutputMode = useStore((s) => s.setOutputMode);
  const setUserOutputDir = useStore((s) => s.setUserOutputDir);
  const setDryRun = useStore((s) => s.setDryRun);
  const setBackup = useStore((s) => s.setBackup);
  const setSkipUnchanged = useStore((s) => s.setSkipUnchanged);
  const setVerifyOutput = useStore((s) => s.setVerifyOutput);
  const cancel = useStore((s) => s.cancelExecution);
  const setProgress = useStore((s) => s.setProgress);

  // 执行进度由 Rust 侧通过事件推送（规范 6.7）
  useEffect(() => {
    const unlisten = listen<ProgressEvent>("meowfit://progress", (event) => {
      setProgress(event.payload);
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, [setProgress]);

  if (!settings) return null;

  const effectiveDir = outputMode === "user" ? userOutputDir : outputDir;

  return (
    <section className="space-y-3 border-t border-border pt-3">
      <div className="flex items-center gap-2">
        <h3 className="text-[11px] font-semibold text-muted">输出与执行</h3>
        {dryRun && <Tag tone="warn">干跑</Tag>}
      </div>

      {/* 两种输出目录方式并存（规范 6.5） */}
      <Field label="输出目录">
        <Select
          value={outputMode}
          onChange={setOutputMode}
          options={[
            { value: "sibling", label: "跟随源文件夹（同级 output/）" },
            { value: "user", label: "每次任务由我指定" },
          ]}
        />
      </Field>

      {outputMode === "user" && (
        <div className="flex items-center gap-2">
          <Button
            onClick={() => {
              void pickOutputFolder().then((dir) => {
                if (dir) setUserOutputDir(dir);
              });
            }}
          >
            选择目录
          </Button>
          <span className="min-w-0 flex-1 truncate font-mono text-[11px] text-faint" title={userOutputDir ?? ""}>
            {userOutputDir ?? "尚未选择"}
          </span>
        </div>
      )}

      {effectiveDir && (
        <p className="truncate font-mono text-[11px] text-faint" title={effectiveDir}>
          将输出到：{effectiveDir}
        </p>
      )}

      <Field label="同名文件">
        <Select
          value={settings.output.onConflict}
          onChange={(value) => void setOutput({ onConflict: value as "skip" | "overwrite" | "rename" })}
          options={[
            { value: "skip", label: "跳过（默认）" },
            { value: "overwrite", label: "覆盖" },
            { value: "rename", label: "自动重命名（追加 -1）" },
          ]}
        />
      </Field>

      <Checkbox checked={settings.output.keepStructure} onChange={(v) => void setOutput({ keepStructure: v })}>
        保留相对目录结构
      </Checkbox>

      <Checkbox checked={verifyOutput} onChange={setVerifyOutput}>
        写出后校验输出能否正常解码（不合格的会删除并计入失败）
      </Checkbox>

      <Checkbox checked={skipUnchanged} onChange={setSkipUnchanged}>
        跳过未变化的素材（增量处理）
      </Checkbox>

      <Checkbox checked={backup} onChange={setBackup}>
        处理前备份源文件到 output/.meowfit-backup/
      </Checkbox>

      <Checkbox checked={dryRun} onChange={setDryRun}>
        干跑模式（只计算与预览，不写出任何文件）
      </Checkbox>

      {settings.output.overwriteSource && (
        <div className="rounded-md border border-danger/40 bg-danger-soft px-2.5 py-2 text-[11px] leading-relaxed text-danger">
          已开启「覆盖源文件」：执行时会直接改写原始文件。强烈建议同时开启备份。
        </div>
      )}

      {diskFree !== null && (
        <p className="text-[11px] text-faint">
          目标磁盘剩余空间：{Math.round(diskFree / 1024 / 1024)} MB
        </p>
      )}

      {running && (
        <div className="space-y-1.5 rounded-md border border-border bg-surface-2 px-2.5 py-2">
          <div className="flex items-center gap-2 text-[11px]">
            <span className="text-muted">
              {progress ? `${progress.done} / ${progress.total}` : "准备中…"}
            </span>
            <span className="min-w-0 flex-1 truncate font-mono text-faint" title={progress?.current ?? ""}>
              {progress?.current ?? ""}
            </span>
            <Button onClick={() => void cancel()}>取消</Button>
          </div>
          <div className="h-1 w-full overflow-hidden rounded bg-surface-3">
            <div
              className="h-full bg-accent transition-all"
              style={{
                width: progress && progress.total > 0 ? `${(progress.done / progress.total) * 100}%` : "0%",
              }}
            />
          </div>
        </div>
      )}

      <Field label="图片目标体积上限（KB）" hint="留空表示不限制；填写后按阶梯逐级逼近，不会无限尝试">
        <TextInput
          value={settings.output.targetBytesKb ? String(settings.output.targetBytesKb) : ""}
          placeholder="例如 500"
          onChange={(text) => {
            const kb = Number(text.trim());
            void setOutput({
              targetBytesKb: text.trim() === "" || !Number.isFinite(kb) || kb <= 0 ? null : Math.round(kb),
            });
          }}
        />
      </Field>
    </section>
  );
}
