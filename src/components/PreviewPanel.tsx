import { useStore } from "../store";
import { MODE_SHORT, SOURCE_LABEL, formatBytes } from "../types";
import { Badge, EmptyHint, RegionTitle } from "./ui";

export default function PreviewPanel() {
  const plan = useStore((s) => s.plan);
  const files = useStore((s) => s.files);
  const outputDir = useStore((s) => s.outputDir);

  if (files.length === 0) {
    return (
      <div className="flex min-h-0 flex-col border-b border-border">
        <RegionTitle>预览区</RegionTitle>
        <EmptyHint>载入素材后在此预览每个文件的结果。</EmptyHint>
      </div>
    );
  }

  const entries = plan?.entries ?? [];
  const toResize = entries.filter((e) => e.action === "resize" && !e.error);
  const unchanged = entries.filter((e) => e.action === "unchanged" && !e.error);
  const errored = entries.filter((e) => e.error);
  const skippedAtScan = files.filter((f) => f.skipReason !== null);
  const sizeById = new Map(files.map((f) => [f.id, f.size]));

  return (
    <div className="flex min-h-0 flex-col border-b border-border">
      <RegionTitle
        right={
          plan ? (
            plan.ok ? (
              <Badge tone="ok">校验通过</Badge>
            ) : (
              <Badge tone="danger">校验失败，不会写出任何文件</Badge>
            )
          ) : null
        }
      >
        预览区
      </RegionTitle>

      <div className="min-h-0 flex-1 overflow-auto px-3 py-2">
        <div className="mb-2 flex flex-wrap items-center gap-2 text-[11px]">
          <Badge tone="accent">将改动 {toResize.length}</Badge>
          <Badge>未改动 {unchanged.length}</Badge>
          <Badge tone="warn">已跳过 {skippedAtScan.length}</Badge>
          {errored.length > 0 && <Badge tone="danger">校验失败 {errored.length}</Badge>}
        </div>

        {errored.length > 0 && (
          <div className="mb-2 rounded border border-danger/40 bg-danger-soft px-2 py-1.5">
            <p className="mb-1 text-[11px] font-semibold text-danger">
              以下素材无法计算，请修正后重试（规范 13.1：存在校验失败时不执行任何写出）
            </p>
            <ul className="space-y-0.5">
              {errored.map((entry) => (
                <li key={entry.id} className="flex gap-2 text-[11px]">
                  <span className="font-mono text-muted">{entry.name}</span>
                  <span className="text-danger">
                    {entry.error?.code} {entry.error?.message}
                  </span>
                </li>
              ))}
            </ul>
          </div>
        )}

        {toResize.length === 0 && errored.length === 0 ? (
          <p className="rounded border border-border bg-surface-2 px-2 py-2 text-[11px] text-faint">
            当前没有任何素材会被改动，执行后将原地不动、也不写出。
            如需处理，请在上方为整体、分组或单个文件设置缩放参数。
          </p>
        ) : (
          <table className="w-full border-collapse text-[11px]">
            <thead className="text-faint">
              <tr>
                <th className="px-1 py-0.5 text-left font-normal">文件</th>
                <th className="px-1 py-0.5 text-left font-normal">原尺寸</th>
                <th className="px-1 py-0.5 text-left font-normal">目标</th>
                <th className="px-1 py-0.5 text-left font-normal">方式</th>
                <th className="px-1 py-0.5 text-left font-normal">来源</th>
                <th className="px-1 py-0.5 text-left font-normal">原体积</th>
              </tr>
            </thead>
            <tbody>
              {toResize.map((entry) => (
                <tr key={entry.id} className="border-t border-border/60">
                  <td className="max-w-[150px] truncate px-1 py-0.5 font-mono" title={entry.name}>
                    {entry.name}
                  </td>
                  <td className="px-1 py-0.5 font-mono text-muted">
                    {entry.originalWidth}×{entry.originalHeight}
                  </td>
                  <td className="px-1 py-0.5 font-mono text-text">
                    {entry.target ? `${entry.target.width}×${entry.target.height}` : "—"}
                    {entry.target &&
                      (entry.target.contentWidth !== entry.target.width ||
                        entry.target.contentHeight !== entry.target.height) && (
                        <span className="ml-1 text-faint">
                          内容 {entry.target.contentWidth}×{entry.target.contentHeight}
                        </span>
                      )}
                  </td>
                  <td className="px-1 py-0.5 text-muted">{entry.mode ? MODE_SHORT[entry.mode] : "—"}</td>
                  <td className="px-1 py-0.5 text-muted">{SOURCE_LABEL[entry.source]}</td>
                  <td className="px-1 py-0.5 font-mono text-faint">
                    {formatBytes(sizeById.get(entry.id))}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}

        {unchanged.length > 0 && (
          <details className="mt-2">
            <summary className="cursor-pointer text-[11px] text-muted">
              未改动 {unchanged.length} 个（原地不动，不写出、不复制）
            </summary>
            <ul className="mt-1 space-y-0.5">
              {unchanged.map((entry) => (
                <li key={entry.id} className="flex gap-2 text-[11px] text-faint">
                  <span className="font-mono">{entry.name}</span>
                  <span>
                    {entry.originalWidth}×{entry.originalHeight}
                  </span>
                </li>
              ))}
            </ul>
          </details>
        )}

        {outputDir && (
          <p className="mt-2 truncate font-mono text-[11px] text-faint" title={outputDir}>
            输出目录：{outputDir}
          </p>
        )}

        <p className="mt-1 text-[11px] text-faint">
          预计输出体积与磁盘空间预检需先有编码结果，随 P5 阶段一并接入。
        </p>
      </div>
    </div>
  );
}
