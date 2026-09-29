import { useStore } from "../store";
import { STATUS_LABEL, formatBytes, type Status } from "../types";
import { Button, EmptyHint, Panel, Tag } from "./ui";

const STATUS_TONE: Record<Status, "ok" | "neutral" | "warn" | "danger"> = {
  success: "ok",
  unchanged: "neutral",
  skipped: "warn",
  failed: "danger",
};

export default function RunPanel({
  className = "",
  height,
}: {
  className?: string;
  /** 由分隔条拖拽出来的高度（个性化布局） */
  height?: number;
}) {
  const plan = useStore((s) => s.plan);
  const files = useStore((s) => s.files);
  const report = useStore((s) => s.report);
  const running = useStore((s) => s.running);
  const execute = useStore((s) => s.execute);

  const toResize = (plan?.entries ?? []).filter((e) => e.action === "resize" && !e.error).length;
  const blocked = plan ? !plan.ok : false;
  const canRun = files.length > 0 && toResize > 0 && !blocked && !running;

  return (
    <Panel
      title="执行与进度区"
      dataPanelId="run"
      className={className}
      height={height}
      right={
        <Button variant="primary" disabled={!canRun} onClick={() => void execute()}>
          {running ? "执行中…" : "执行"}
        </Button>
      }
    >
      <div className="panel-body">
        {files.length === 0 ? (
          <EmptyHint>
            <span>载入素材后可在此执行</span>
          </EmptyHint>
        ) : (
          <>
            <p className="text-[11px] text-faint">
              {blocked
                ? "存在校验失败的素材，已禁止执行"
                : toResize === 0
                  ? "没有需要改动的素材"
                  : `将处理 ${toResize} 个素材`}
            </p>

            {report && (
              <>
                <div className="mt-2.5 flex flex-wrap gap-1.5">
                  <Tag>总数 {report.counts.total}</Tag>
                  <Tag tone="ok">成功 {report.counts.success}</Tag>
                  <Tag>未改动 {report.counts.unchanged}</Tag>
                  <Tag tone="warn">已跳过 {report.counts.skipped}</Tag>
                  <Tag tone={report.counts.failed > 0 ? "danger" : "neutral"}>
                    失败 {report.counts.failed}
                  </Tag>
                </div>

                {(() => {
                  // 体积对比（规范 6.8：格式转换后须给出体积对比）
                  const measured = report.outcomes.filter((o) => o.newSize !== undefined);
                  if (measured.length === 0) return null;
                  const before = measured.reduce((sum, o) => sum + o.originalSize, 0);
                  const after = measured.reduce((sum, o) => sum + (o.newSize ?? 0), 0);
                  const percent = before > 0 ? ((after - before) / before) * 100 : 0;
                  return (
                    <p className="mt-2 text-[11px] text-muted">
                      体积对比{" "}
                      <span className="font-mono">
                        {formatBytes(before)} → {formatBytes(after)}
                      </span>
                      <span className={percent <= 0 ? " text-ok" : " text-warn"}>
                        {" "}
                        ({percent > 0 ? "+" : ""}
                        {percent.toFixed(1)}%)
                      </span>
                    </p>
                  );
                })()}

                {report.counts.failed > 0 && (
                  <div className="mt-2.5 rounded-md border border-danger/40 bg-danger-soft px-2.5 py-2">
                    <p className="mb-1.5 text-[11px] font-semibold text-danger">失败清单</p>
                    <ul className="space-y-1">
                      {report.outcomes
                        .filter((o) => o.status === "failed")
                        .map((o) => (
                          <li key={o.id} className="flex gap-2 text-[11px]">
                            <span className="truncate font-mono text-muted">{o.id}</span>
                            <span className="shrink-0 text-danger">{o.reason}</span>
                          </li>
                        ))}
                    </ul>
                  </div>
                )}

                <details className="fold mt-2.5 px-0" open={report.counts.success > 0}>
                  <summary>全部结果（{report.outcomes.length}）</summary>
                  <table className="grid-table mt-1.5">
                    <tbody>
                      {report.outcomes.map((o) => (
                        <tr key={o.id} className="row-idle">
                          <td className="max-w-[150px] truncate font-mono text-[11px]" title={o.id}>
                            {o.id}
                          </td>
                          <td className="whitespace-nowrap">
                            <Tag tone={STATUS_TONE[o.status]}>{STATUS_LABEL[o.status]}</Tag>
                          </td>
                          <td className="font-mono text-[11px] whitespace-nowrap text-faint">
                            {o.newSize !== undefined
                              ? `${formatBytes(o.originalSize)} → ${formatBytes(o.newSize)}`
                              : formatBytes(o.originalSize)}
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </details>
              </>
            )}
          </>
        )}
      </div>
    </Panel>
  );
}
