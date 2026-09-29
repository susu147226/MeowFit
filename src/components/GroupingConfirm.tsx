import { useStore } from "../store";
import { GROUPING_LABEL } from "../types";
import { Button } from "./ui";

/** 切换分组方式时，若已存在手动修正，须提示修正将被重置并等待确认（规范 6.4 / 场景 6）。 */
export default function GroupingConfirm() {
  const pendingGrouping = useStore((s) => s.pendingGrouping);
  const grouping = useStore((s) => s.grouping);
  const groupsDirty = useStore((s) => s.groupsDirty);
  const confirmGrouping = useStore((s) => s.confirmGrouping);
  const cancelGrouping = useStore((s) => s.cancelGrouping);

  if (!pendingGrouping || !groupsDirty) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-6">
      <div className="w-[420px] rounded-lg border border-border bg-surface p-5 shadow-xl">
        <h2 className="text-[14px] font-semibold">切换分组方式将重置手动修正</h2>
        <p className="mt-2 text-[12px] leading-relaxed text-muted">
          你已对分组做过手动修正（重命名、移动文件或合并分组）。切换到
          「{GROUPING_LABEL[pendingGrouping]}」会按新方式重新自动分组，
          这些修正将被<strong className="text-text">全部重置</strong>。
        </p>
        <p className="mt-1 text-[11px] text-faint">
          当前方式：{GROUPING_LABEL[grouping]} → 目标方式：{GROUPING_LABEL[pendingGrouping]}
        </p>
        <div className="mt-4 flex justify-end gap-2">
          <Button onClick={cancelGrouping}>取消</Button>
          <Button variant="primary" onClick={() => void confirmGrouping()}>
            确认并重置
          </Button>
        </div>
      </div>
    </div>
  );
}
