import { useStore } from "../store";
import { formatBytes } from "../types";
import { Button, Tag } from "./ui";

/**
 * 覆盖源文件的二次确认（规范 6.5 / 第八节）。
 * 文案列出将被覆盖的文件数量与路径；取消则不执行任何操作。
 */
export default function OverwriteConfirm() {
  const open = useStore((s) => s.overwriteConfirmOpen);
  const plan = useStore((s) => s.plan);
  const files = useStore((s) => s.files);
  const confirm = useStore((s) => s.confirmOverwriteAndRun);
  const close = useStore((s) => s.setOverwriteConfirmOpen);

  if (!open) return null;

  const targets = (plan?.entries ?? []).filter((e) => e.action === "resize");
  const sizeById = new Map(files.map((f) => [f.id, f.size]));
  const total = targets.reduce((sum, e) => sum + (sizeById.get(e.id) ?? 0), 0);

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-6">
      <div className="flex max-h-full w-[520px] flex-col rounded-lg border border-border bg-surface p-5 shadow-xl">
        <h2 className="text-[14px] font-semibold text-danger">确认覆盖源文件？</h2>
        <p className="mt-2 text-[12px] leading-relaxed text-muted">
          即将<strong className="text-danger">直接改写原始文件</strong>，共{" "}
          <span className="font-mono">{targets.length}</span> 个，合计{" "}
          <span className="font-mono">{formatBytes(total)}</span>。
        </p>
        <p className="mt-1 text-[11px] text-faint">
          建议先在「输出与执行」里开启「处理前备份源文件」，否则原图将被替换且无法找回。
        </p>

        <div className="mt-3 min-h-0 flex-1 overflow-auto rounded border border-border bg-surface-2 p-2">
          <ul className="space-y-1">
            {targets.map((entry) => (
              <li key={entry.id} className="flex items-center gap-2 text-[11px]">
                <span className="min-w-0 flex-1 truncate font-mono text-muted" title={entry.id}>
                  {entry.id}
                </span>
                <Tag>{formatBytes(sizeById.get(entry.id))}</Tag>
              </li>
            ))}
          </ul>
        </div>

        <div className="mt-4 flex justify-end gap-2">
          <Button onClick={() => close(false)}>取消（不做任何操作）</Button>
          <Button variant="primary" onClick={() => void confirm()}>
            确认覆盖并执行
          </Button>
        </div>
      </div>
    </div>
  );
}
