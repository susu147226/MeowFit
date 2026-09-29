import { useStore } from "../store";
import { STATUS_LABEL, formatBytes, type Status } from "../types";
import { Badge, Button, EmptyHint, RegionTitle } from "./ui";

const STATUS_TONE: Record<Status, "ok" | "neutral" | "warn" | "danger"> = {
  success: "ok",
  unchanged: "neutral",
  skipped: "warn",
  failed: "danger",
};

export default function RunPanel() {
  const plan = useStore((s) => s.plan);
  const files = useStore((s) => s.files);
  const report = useStore((s) => s.report);
  const running = useStore((s) => s.running);
  const execute = useStore((s) => s.execute);

  const toResize = (plan?.entries ?? []).filter((e) => e.action === "resize" && !e.error).length;
  const blocked = plan ? !plan.ok : false;
  const canRun = files.length > 0 && toResize > 0 && !blocked && !running;

  return (
    <div className="flex min-h-0 flex-col">
      <RegionTitle
        right={
          <Button variant="primary" disabled={!canRun} onClick={() => void execute()}>
            {running ? "执行中…" : "执行"}
          </Button>
        }
      >
        执行与进度区
      </RegionTitle>

      <div className="min-h-0 flex-1 overflow-auto px-3 py-2">
        {files.length === 0 ? (
          <EmptyHint>载入素材后可在此执行。</EmptyHint>
        ) : (
          <>
            <p className="text-[11px] text-faint">
              {blocked
                ? "存在校验失败的素材，已禁止执行。"
                : toResize === 0
                  ? "没有需要改动的素材，无需执行。"
                  : `将处理 ${toResize} 个素材，其余保持原样。`}
            </p>

            {report && (
              <>
                <div className="mt-2 flex flex-wrap gap-1.5">
                  <Badge>总数 {report.counts.total}</Badge>
                  <Badge tone="ok">成功 {report.counts.success}</Badge>
                  <Badge>未改动 {report.counts.unchanged}</Badge>
                  <Badge tone="warn">已跳过 {report.counts.skipped}</Badge>
                  <Badge tone={report.counts.failed > 0 ? "danger" : "neutral"}>
                    失败 {report.counts.failed}
                  </Badge>
                </div>

                <p className="mt-1 truncate font-mono text-[11px] text-faint" title={report.outputDir}>
                  输出目录：{report.outputDir}
                </p>

                {report.counts.failed > 0 && (
                  <div className="mt-2 rounded border border-danger/40 bg-danger-soft px-2 py-1.5">
                    <p className="mb-1 text-[11px] font-semibold text-danger">失败清单</p>
                    <ul className="space-y-0.5">
                      {report.outcomes
                        .filter((o) => o.status === "failed")
                        .map((o) => (
                          <li key={o.id} className="flex gap-2 text-[11px]">
                            <span className="font-mono text-muted">{o.id}</span>
                            <span className="text-danger">{o.reason}</span>
                          </li>
                        ))}
                    </ul>
                  </div>
                )}

                <details className="mt-2" open={report.counts.success > 0}>
                  <summary className="cursor-pointer text-[11px] text-muted">
                    全部结果（{report.outcomes.length}）
                  </summary>
                  <table className="mt-1 w-full border-collapse text-[11px]">
                    <tbody>
                      {report.outcomes.map((o) => (
                        <tr key={o.id} className="border-t border-border/60">
                          <td className="max-w-[180px] truncate px-1 py-0.5 font-mono" title={o.id}>
                            {o.id}
                          </td>
                          <td className="px-1 py-0.5">
                            <Badge tone={STATUS_TONE[o.status]}>{STATUS_LABEL[o.status]}</Badge>
                          </td>
                          <td className="px-1 py-0.5 font-mono text-faint">
                            {o.newSize !== undefined
                              ? `${formatBytes(o.originalSize)} → ${formatBytes(o.newSize)}`
                              : formatBytes(o.originalSize)}
                          </td>
                          <td className="max-w-[160px] truncate px-1 py-0.5 text-faint" title={o.reason ?? ""}>
                            {o.reason ?? ""}
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
    </div>
  );
}
