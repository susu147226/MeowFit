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
import { EmptyHint, Panel, Select, Tag } from "./ui";

const KIND_LABEL: Record<MediaKind, string> = {
  raster: "静态图片",
  svg: "SVG",
  animated: "动图",
  video: "视频",
};

const GROUPINGS: Grouping[] = ["prefix", "extension", "folder"];

export default function FileList({ className = "" }: { className?: string }) {
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

  const viewToggle = (
    <div className="flex overflow-hidden rounded-md border border-border">
      {(["groups", "files"] as const).map((mode) => (
        <button
          key={mode}
          type="button"
          onClick={() => setView(mode)}
          className={`px-2.5 py-1 text-[11px] transition ${
            view === mode
              ? "bg-accent text-accent-contrast"
              : "bg-surface text-muted hover:bg-surface-3 hover:text-text"
          }`}
        >
          {mode === "groups" ? "按分组" : "按文件"}
        </button>
      ))}
    </div>
  );

  if (files.length === 0) {
    return (
      <Panel title="素材列表区" className={className}>
        <EmptyHint>
          <span>选择或拖入一个文件夹以开始扫描</span>
        </EmptyHint>
      </Panel>
    );
  }

  const renderStatus = (entry: PlanEntry | undefined, id: string) => {
    const outcome = outcomeById.get(id);
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
        <span className="flex items-center gap-1.5">
          <Tag tone={tone}>{label}</Tag>
          {outcome.reason && (
            <span className="max-w-[180px] truncate text-[11px] text-faint" title={outcome.reason}>
              {outcome.reason}
            </span>
          )}
        </span>
      );
    }
    if (!entry) return <span className="text-[11px] text-faint">—</span>;
    if (entry.error) return <Tag tone="danger">{entry.error.message}</Tag>;
    if (entry.action === "unchanged") return <Tag>未改动</Tag>;
    return (
      <Tag tone="accent">
        {entry.target ? `${entry.target.width}×${entry.target.height}` : "待缩放"}
      </Tag>
    );
  };

  return (
    <Panel
      title="素材列表区"
      className={className}
      right={
        <>
          <span className="text-[11px] text-faint">
            {stats.total} 个 · 可处理 {stats.processable} · 跳过 {stats.skipped}
          </span>
          {viewToggle}
        </>
      }
    >
      {/* 分组方式：三种单选，不允许叠加（规范 6.4） */}
      <div className="flex shrink-0 flex-wrap items-center gap-1.5 border-b border-border bg-surface px-3.5 py-2">
        {GROUPINGS.map((option) => (
          <button
            key={option}
            type="button"
            onClick={() => requestGrouping(option)}
            className={`rounded-md border px-2 py-0.5 text-[11px] transition ${
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
            <Tag key={kind}>
              {kind} {count}
            </Tag>
          ))}
        </span>
      </div>

      <div className="min-h-0 flex-1 overflow-auto">
        {view === "files" ? (
          <table className="grid-table">
            <thead>
              <tr>
                <th>文件</th>
                <th>类型</th>
                <th>原尺寸</th>
                <th>分组</th>
                <th>来源</th>
                <th>方式</th>
                <th>状态 / 目标</th>
                <th>移动到</th>
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
                    className={`cursor-pointer ${
                      active ? "row-active" : selected ? "row-selected" : "row-idle"
                    }`}
                  >
                    <td className="max-w-[280px] truncate font-mono text-[11px]" title={file.relativePath}>
                      {file.relativePath}
                    </td>
                    <td className="text-muted">
                      {file.kind ? KIND_LABEL[file.kind] : <Tag tone="warn">不支持</Tag>}
                    </td>
                    <td className="font-mono text-[11px] text-muted">
                      {file.width && file.height ? `${file.width}×${file.height}` : "—"}
                    </td>
                    <td className="text-muted">{groupName || "—"}</td>
                    <td className="text-muted">{entry ? SOURCE_LABEL[entry.source] : "—"}</td>
                    <td className="text-muted">{entry?.mode ? MODE_SHORT[entry.mode] : "不变"}</td>
                    <td>{renderStatus(entry, file.id)}</td>
                    <td>
                      <span onClick={(e) => e.stopPropagation()}>
                        <Select
                          value=""
                          onChange={(value) => {
                            if (value) moveFile(file.id, value);
                          }}
                          options={[
                            { value: "", label: "移至…" },
                            ...groups
                              .filter((g) => g.name !== groupName)
                              .map((g) => ({ value: g.name, label: g.name })),
                          ]}
                        />
                      </span>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        ) : (
          <table className="grid-table">
            <thead>
              <tr>
                <th>分组</th>
                <th>文件数</th>
                <th>类型</th>
                <th>生效设置来源</th>
                <th>将被改为</th>
                <th>操作</th>
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
                    className={`cursor-pointer ${
                      active ? "row-active" : isMerging ? "bg-warn-soft" : "row-idle"
                    }`}
                  >
                    <td>
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
                          className="input"
                        />
                      ) : (
                        <span className="font-medium">{group.name}</span>
                      )}
                    </td>
                    <td className="text-muted">{group.fileIds.length}</td>
                    <td className="text-muted">{kinds.join("、") || "—"}</td>
                    <td className="text-muted">
                      {changed.length === 0 ? "不变" : sources.join("、")}
                    </td>
                    <td>
                      {changed.length === 0 ? (
                        <Tag>原地不动</Tag>
                      ) : (
                        <Tag tone="accent">
                          {changed.length} 个 → {preview ? `${preview.width}×${preview.height}` : "—"}
                        </Tag>
                      )}
                    </td>
                    <td>
                      <span
                        className="flex items-center gap-1"
                        onClick={(e) => e.stopPropagation()}
                      >
                        <button
                          type="button"
                          className="btn btn-ghost px-1.5 py-0"
                          onClick={() => {
                            setRenaming(group.name);
                            setRenameText(group.name);
                          }}
                        >
                          重命名
                        </button>
                        <button
                          type="button"
                          className="btn btn-ghost px-1.5 py-0"
                          onClick={() => setMergeFrom(isMerging ? null : group.name)}
                        >
                          {isMerging ? "取消" : "合并"}
                        </button>
                        {isMerging && (
                          <Select
                            value=""
                            onChange={(value) => {
                              if (value) mergeGroups(group.name, value);
                              setMergeFrom(null);
                            }}
                            options={[
                              { value: "", label: "合并到…" },
                              ...groups
                                .filter((g) => g.name !== group.name)
                                .map((g) => ({ value: g.name, label: g.name })),
                            ]}
                          />
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
          <details className="fold">
            <summary>已跳过 {stats.skipped} 个文件</summary>
            <ul className="mt-1.5 space-y-1">
              {files
                .filter((f) => f.skipReason !== null)
                .map((f) => (
                  <li key={f.id} className="flex gap-2 text-[11px]">
                    <span className="truncate font-mono text-muted">{f.relativePath}</span>
                    <span className="shrink-0 text-faint">
                      {f.skipReason} · {formatBytes(f.size)}
                    </span>
                  </li>
                ))}
            </ul>
          </details>
        )}
      </div>
    </Panel>
  );
}
