import { useStore } from "../store";
import { MODE_SHORT, SOURCE_LABEL, formatBytes } from "../types";
import { EmptyHint, Panel, Tag } from "./ui";

export default function PreviewPanel({ className = "" }: { className?: string }) {
  const plan = useStore((s) => s.plan);
  const files = useStore((s) => s.files);
  const outputDir = useStore((s) => s.outputDir);

  if (files.length === 0) {
    return (
      <Panel title="预览区" className={className}>
        <EmptyHint>
          <span>载入素材后在此预览结果</span>
        </EmptyHint>
      </Panel>
    );
  }

  const entries = plan?.entries ?? [];
  const toResize = entries.filter((e) => e.action === "resize" && !e.error);
  const unchanged = entries.filter((e) => e.action === "unchanged" && !e.error);
  const errored = entries.filter((e) => e.error);
  const skippedAtScan = files.filter((f) => f.skipReason !== null);
  const sizeById = new Map(files.map((f) => [f.id, f.size]));

  return (
    <Panel
      title="预览区"
      className={className}
      right={
        plan ? (
          plan.ok ? (
            <Tag tone="ok">校验通过</Tag>
          ) : (
            <Tag tone="danger">校验失败</Tag>
          )
        ) : null
      }
    >
      <div className="panel-body">
        <div className="mb-3 flex flex-wrap items-center gap-1.5">
          <Tag tone="accent">将改动 {toResize.length}</Tag>
          <Tag>未改动 {unchanged.length}</Tag>
          <Tag tone="warn">已跳过 {skippedAtScan.length}</Tag>
          {errored.length > 0 && <Tag tone="danger">校验失败 {errored.length}</Tag>}
        </div>

        {errored.length > 0 && (
          <div className="mb-3 rounded-md border border-danger/40 bg-danger-soft px-2.5 py-2">
            <p className="mb-1.5 text-[11px] font-semibold text-danger">以下素材无法计算，请修正后重试</p>
            <ul className="space-y-1">
              {errored.map((entry) => (
                <li key={entry.id} className="flex gap-2 text-[11px]">
                  <span className="truncate font-mono text-muted" title={entry.name}>
                    {entry.name}
                  </span>
                  <span className="shrink-0 text-danger">
                    {entry.error?.code} {entry.error?.message}
                  </span>
                </li>
              ))}
            </ul>
          </div>
        )}

        {toResize.length === 0 && errored.length === 0 ? (
          <p className="rounded-md border border-border bg-surface-2 px-2.5 py-3 text-center text-[12px] text-faint">
            没有素材会被改动
          </p>
        ) : (
          <table className="grid-table">
            <thead>
              <tr>
                <th>文件</th>
                <th>原尺寸</th>
                <th>目标</th>
                <th>方式</th>
                <th>来源</th>
                <th>原体积</th>
              </tr>
            </thead>
            <tbody>
              {toResize.map((entry) => (
                <tr key={entry.id} className="row-idle">
                  <td className="max-w-[130px] truncate font-mono text-[11px]" title={entry.name}>
                    {entry.name}
                  </td>
                  <td className="font-mono text-[11px] whitespace-nowrap text-muted">
                    {entry.originalWidth}×{entry.originalHeight}
                  </td>
                  <td className="font-mono text-[11px] whitespace-nowrap">
                    {entry.target ? `${entry.target.width}×${entry.target.height}` : "—"}
                  </td>
                  <td className="whitespace-nowrap text-muted">
                    {entry.mode ? MODE_SHORT[entry.mode] : "—"}
                  </td>
                  <td className="whitespace-nowrap text-muted">{SOURCE_LABEL[entry.source]}</td>
                  <td className="font-mono text-[11px] whitespace-nowrap text-faint">
                    {formatBytes(sizeById.get(entry.id))}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}

        {unchanged.length > 0 && (
          <details className="fold mt-3">
            <summary>未改动 {unchanged.length} 个</summary>
            <ul className="mt-1.5 space-y-1">
              {unchanged.map((entry) => (
                <li key={entry.id} className="flex gap-2 text-[11px] text-faint">
                  <span className="truncate font-mono">{entry.name}</span>
                  <span className="shrink-0">
                    {entry.originalWidth}×{entry.originalHeight}
                  </span>
                </li>
              ))}
            </ul>
          </details>
        )}

        {outputDir && (
          <p className="mt-3 truncate font-mono text-[11px] text-faint" title={outputDir}>
            输出 {outputDir}
          </p>
        )}
      </div>
    </Panel>
  );
}
