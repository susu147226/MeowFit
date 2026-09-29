import { useMemo, useState } from "react";

import { processable, useStore } from "../store";
import {
  GROUPING_LABEL,
  MODE_SHORT,
  SOURCE_LABEL,
  formatBytes,
  type Grouping,
  type MediaKind,
  type PlanEntry,
} from "../types";
import { Badge, Button, EmptyHint, RegionTitle } from "./ui";

const KIND_LABEL: Record<MediaKind, string> = {
  raster: "静态图片",
  svg: "SVG",
  animated: "动图",
  video: "视频",
};

const GROUPINGS: Grouping[] = ["prefix", "extension", "folder"];

export default function FileList() {
  const files = useStore((s) => s.files);
  const groups = useStore((s) => s.groups);
  const grouping = useStore((s) => s.grouping);
  const plan = useStore((s) => s.plan);
  const report = useStore((s) => s.report);
  const scope = useStore((s) => s.scope);
  const selectedIds = useStore((s) => s.selectedIds);
  const setScope = useStore((s) => s.setScope);
  const toggleSelect = useStore((s) => s.toggleSelect);
  const requestGrouping = useStore((s) => s.requestGrouping);
  const renameGroup = useStore((s) => s.renameGroup);
  const mergeGroups = useStore((s) => s.mergeGroups);
  const moveFile = useStore((s) => s.moveFile);

  const [view, setView] = useState<"files" | "groups">("groups");
  const [renaming, setRenaming] = useState<string | null>(null);
  const [renameText, setRenameText] = useState("");
  const [mergeFrom, setMergeFrom] = useState<string | null>(null);

  const planById = useMemo(() => {
    const map = new Map<string, PlanEntry>();
    for (const entry of plan?.entries ?? []) map.set(entry.id, entry);
    return map;
  }, [plan]);

  const outcomeById = useMemo(() => {
    const map = new Map<string, { status: string; reason?: string }>();
    for (const o of report?.outcomes ?? []) map.set(o.id, { status: o.status, reason: o.reason });
    return map;
  }, [report]);

  const stats = useMemo(() => {
    const byKind = new Map<string, number>();
    for (const f of files) {
      const key = f.kind ? KIND_LABEL[f.kind] : "不支持";
      byKind.set(key, (byKind.get(key) ?? 0) + 1);
    }
    return {
      total: files.length,
      processable: processable(files).length,
      skipped: files.filter((f) => f.skipReason !== null).length,
      byKind: [...byKind.entries()].sort((a, b) => b[1] - a[1]),
    };
  }, [files]);

  if (files.length === 0) {
    return (
      <div className="flex min-h-0 flex-1 flex-col">
        <RegionTitle>素材列表区</RegionTitle>
        <EmptyHint>
          选择、拖入或粘贴一个文件夹路径以开始。
          <br />
          扫描是只读的，在你确认执行前不会改动任何文件。
        </EmptyHint>
      </div>
    );
  }

  const renderEntry = (entry: PlanEntry | undefined, fallbackId: string) => {
    const outcome = outcomeById.get(fallbackId);
    if (outcome) {
      const tone =
        outcome.status === "success"
          ? "ok"
          : outcome.status === "failed"
            ? "danger"
            : outcome.status === "skipped"
              ? "warn"
              : "neutral";
      const label =
        outcome.status === "success"
          ? "成功"
          : outcome.status === "failed"
            ? "失败"
            : outcome.status === "skipped"
              ? "已跳过"
              : "未改动";
      return (
        <span className="flex items-center gap-1">
          <Badge tone={tone}>{label}</Badge>
          {outcome.reason && (
            <span className="max-w-[160px] truncate text-[11px] text-faint" title={outcome.reason}>
              {outcome.reason}
            </span>
          )}
        </span>
      );
    }
    if (!entry) return <span className="text-[11px] text-faint">—</span>;
    if (entry.error) {
      return <Badge tone="danger">{entry.error.message}</Badge>;
    }
    if (entry.action === "unchanged") {
      return <Badge tone="neutral">未改动</Badge>;
    }
    return (
      <Badge tone="accent">
        {entry.target ? `${entry.target.width}×${entry.target.height}` : "待缩放"}
      </Badge>
    );
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <RegionTitle
        right={
          <>
            <span className="text-[11px] text-faint">
              共 {stats.total} 个 · 可处理 {stats.processable} · 已跳过 {stats.skipped}
            </span>
            <span className="flex overflow-hidden rounded border border-border">
              <button
                type="button"
                onClick={() => setView("groups")}
                className={`px-2 py-0.5 text-[11px] ${view === "groups" ? "bg-accent text-accent-contrast" : "text-muted hover:text-text"}`}
              >
                按分组
              </button>
              <button
                type="button"
                onClick={() => setView("files")}
                className={`px-2 py-0.5 text-[11px] ${view === "files" ? "bg-accent text-accent-contrast" : "text-muted hover:text-text"}`}
              >
                按文件
              </button>
            </span>
          </>
        }
      >
        素材列表区
      </RegionTitle>

      {/* 分组方式：三种单选，不允许叠加（规范 6.4） */}
      <div className="flex shrink-0 flex-wrap items-center gap-1.5 border-b border-border bg-surface px-3 py-1.5">
        <span className="text-[11px] text-faint">分组方式</span>
        {GROUPINGS.map((option) => (
          <button
            key={option}
            type="button"
            onClick={() => requestGrouping(option)}
            className={`rounded border px-2 py-0.5 text-[11px] transition ${
              grouping === option
                ? "border-accent bg-accent-soft text-accent"
                : "border-border text-muted hover:border-border-strong hover:text-text"
            }`}
          >
            {GROUPING_LABEL[option]}
          </button>
        ))}
        <span className="ml-auto flex flex-wrap gap-1">
          {stats.byKind.map(([kind, count]) => (
            <Badge key={kind}>
              {kind} {count}
            </Badge>
          ))}
        </span>
      </div>

      <div className="min-h-0 flex-1 overflow-auto">
        {view === "files" ? (
          <table className="w-full border-collapse text-[12px]">
            <thead className="sticky top-0 bg-surface-2 text-[11px] text-faint">
              <tr>
                <th className="px-2 py-1 text-left font-normal">文件</th>
                <th className="px-2 py-1 text-left font-normal">类型</th>
                <th className="px-2 py-1 text-left font-normal">原尺寸</th>
                <th className="px-2 py-1 text-left font-normal">分组</th>
                <th className="px-2 py-1 text-left font-normal">生效来源</th>
                <th className="px-2 py-1 text-left font-normal">方式</th>
                <th className="px-2 py-1 text-left font-normal">状态 / 目标</th>
                <th className="px-2 py-1 text-left font-normal">移动到</th>
              </tr>
            </thead>
            <tbody>
              {files.map((file) => {
                const entry = planById.get(file.id);
                const active = scope.type === "file" && scope.id === file.id;
                const selected = selectedIds.includes(file.id);
                const groupName = groups.find((g) => g.fileIds.includes(file.id))?.name ?? "";
                return (
                  <tr
                    key={file.id}
                    onClick={() => {
                      setScope({ type: "file", id: file.id });
                      toggleSelect(file.id, false);
                    }}
                    className={`cursor-pointer border-b border-border/60 ${
                      active ? "bg-accent-soft" : selected ? "bg-surface-3" : "hover:bg-surface-2"
                    }`}
                  >
                    <td className="max-w-[260px] truncate px-2 py-1 font-mono text-[11px]" title={file.relativePath}>
                      {file.relativePath}
                    </td>
                    <td className="px-2 py-1 text-muted">
                      {file.kind ? KIND_LABEL[file.kind] : <Badge tone="warn">不支持</Badge>}
                    </td>
                    <td className="px-2 py-1 font-mono text-[11px] text-muted">
                      {file.width && file.height ? `${file.width}×${file.height}` : "—"}
                    </td>
                    <td className="px-2 py-1 text-muted">{groupName || "—"}</td>
                    <td className="px-2 py-1 text-muted">
                      {entry ? SOURCE_LABEL[entry.source] : "—"}
                    </td>
                    <td className="px-2 py-1 text-muted">
                      {entry?.mode ? MODE_SHORT[entry.mode] : "不变"}
                    </td>
                    <td className="px-2 py-1">{renderEntry(entry, file.id)}</td>
                    <td className="px-2 py-1">
                      <select
                        className="max-w-[120px] rounded border border-border bg-surface px-1 py-0.5 text-[11px]"
                        value=""
                        onClick={(e) => e.stopPropagation()}
                        onChange={(e) => {
                          if (e.target.value) moveFile(file.id, e.target.value);
                        }}
                      >
                        <option value="">移至…</option>
                        {groups
                          .filter((g) => g.name !== groupName)
                          .map((g) => (
                            <option key={g.name} value={g.name}>
                              {g.name}
                            </option>
                          ))}
                      </select>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        ) : (
          <table className="w-full border-collapse text-[12px]">
            <thead className="sticky top-0 bg-surface-2 text-[11px] text-faint">
              <tr>
                <th className="px-2 py-1 text-left font-normal">分组</th>
                <th className="px-2 py-1 text-left font-normal">文件数</th>
                <th className="px-2 py-1 text-left font-normal">类型</th>
                <th className="px-2 py-1 text-left font-normal">生效设置来源</th>
                <th className="px-2 py-1 text-left font-normal">改组将被改为</th>
                <th className="px-2 py-1 text-left font-normal">操作</th>
              </tr>
            </thead>
            <tbody>
              {groups.map((group) => {
                const members = group.fileIds
                  .map((id) => planById.get(id))
                  .filter((e): e is PlanEntry => Boolean(e));
                const changed = members.filter((e) => e.action === "resize");
                const sources = [...new Set(changed.map((e) => SOURCE_LABEL[e.source]))];
                const kinds = [
                  ...new Set(
                    group.fileIds
                      .map((id) => files.find((f) => f.id === id)?.kind)
                      .filter(Boolean)
                      .map((k) => KIND_LABEL[k as MediaKind]),
                  ),
                ];
                const preview = changed[0]?.target;
                const active = scope.type === "group" && scope.name === group.name;
                const isMerging = mergeFrom === group.name;

                return (
                  <tr
                    key={group.name}
                    onClick={() => setScope({ type: "group", name: group.name })}
                    className={`cursor-pointer border-b border-border/60 ${
                      active ? "bg-accent-soft" : isMerging ? "bg-warn-soft" : "hover:bg-surface-2"
                    }`}
                  >
                    <td className="px-2 py-1">
                      {renaming === group.name ? (
                        <input
                          autoFocus
                          value={renameText}
                          onClick={(e) => e.stopPropagation()}
                          onChange={(e) => setRenameText(e.target.value)}
                          onBlur={() => {
                            renameGroup(group.name, renameText);
                            setRenaming(null);
                          }}
                          onKeyDown={(e) => {
                            if (e.key === "Enter") {
                              renameGroup(group.name, renameText);
                              setRenaming(null);
                            }
                            if (e.key === "Escape") setRenaming(null);
                          }}
                          className="w-full rounded border border-accent bg-surface px-1 py-0.5 font-mono text-[11px] outline-none"
                        />
                      ) : (
                        <span className="font-medium">{group.name}</span>
                      )}
                    </td>
                    <td className="px-2 py-1 text-muted">{group.fileIds.length}</td>
                    <td className="px-2 py-1 text-muted">{kinds.join("、") || "—"}</td>
                    <td className="px-2 py-1 text-muted">
                      {changed.length === 0 ? "不变（保持原样）" : sources.join("、")}
                    </td>
                    <td className="px-2 py-1">
                      {changed.length === 0 ? (
                        <Badge tone="neutral">原地不动，不写出</Badge>
                      ) : (
                        <Badge tone="accent">
                          {changed.length} 个 → {preview ? `${preview.width}×${preview.height}` : "—"}
                        </Badge>
                      )}
                    </td>
                    <td className="px-2 py-1">
                      <span className="flex gap-1" onClick={(e) => e.stopPropagation()}>
                        <Button
                          variant="ghost"
                          onClick={() => {
                            setRenaming(group.name);
                            setRenameText(group.name);
                          }}
                        >
                          重命名
                        </Button>
                        <Button
                          variant="ghost"
                          onClick={() => setMergeFrom(isMerging ? null : group.name)}
                        >
                          {isMerging ? "取消合并" : "合并到…"}
                        </Button>
                        {isMerging && (
                          <select
                            autoFocus
                            className="rounded border border-border bg-surface px-1 py-0.5 text-[11px]"
                            value=""
                            onChange={(e) => {
                              if (e.target.value) mergeGroups(group.name, e.target.value);
                              setMergeFrom(null);
                            }}
                          >
                            <option value="">选择目标组</option>
                            {groups
                              .filter((g) => g.name !== group.name)
                              .map((g) => (
                                <option key={g.name} value={g.name}>
                                  {g.name}
                                </option>
                              ))}
                          </select>
                        )}
                      </span>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        )}

        {/* 已跳过清单（规范 5.5 / 场景 8） */}
        {stats.skipped > 0 && (
          <details className="border-t border-border bg-surface-2 px-3 py-1.5">
            <summary className="cursor-pointer text-[12px] text-muted">
              已跳过 {stats.skipped} 个文件
            </summary>
            <ul className="mt-1 space-y-0.5">
              {files
                .filter((f) => f.skipReason !== null)
                .map((f) => (
                  <li key={f.id} className="flex gap-2 text-[11px]">
                    <span className="font-mono text-muted">{f.relativePath}</span>
                    <span className="text-faint">
                      {f.skipReason} · {formatBytes(f.size)}
                    </span>
                  </li>
                ))}
            </ul>
          </details>
        )}
      </div>
    </div>
  );
}
