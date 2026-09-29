import { useState } from "react";

import { useStore } from "../store";
import type { Setting } from "../types";
import { Button, Field, Select, Tag } from "./ui";

/** 预设条（规范 6.12）：套用内置与自定义预设，也可把当前作用域的设置存成新预设。 */
export default function PresetBar({ current }: { current: Setting | null }) {
  const presets = useStore((s) => s.presets);
  const scope = useStore((s) => s.scope);
  const applyPreset = useStore((s) => s.setGlobalSetting);
  const setGroupTier = useStore((s) => s.setGroupTier);
  const setFileSetting = useStore((s) => s.setFileSetting);
  const saveCurrentAsPreset = useStore((s) => s.saveCurrentAsPreset);
  const renamePreset = useStore((s) => s.renamePreset);
  const removePreset = useStore((s) => s.removePreset);

  const [managing, setManaging] = useState(false);

  const visible = presets.filter((p) => !p.hidden);
  if (visible.length === 0) return null;

  /** 预设套用到当前作用域 */
  const apply = (id: string) => {
    const preset = presets.find((p) => p.id === id);
    if (!preset) return;
    if (scope.type === "global") applyPreset(preset.setting);
    else if (scope.type === "group") setGroupTier(scope.name, { kind: "explicit", setting: preset.setting });
    else setFileSetting(scope.id, preset.setting);
  };

  return (
    <Field
      label="预设"
      hint="套用到当前作用域；内置预设可隐藏但不可删除"
    >
      <div className="flex flex-wrap items-center gap-2">
        <Select
          className="max-w-[220px]"
          value=""
          onChange={(id) => {
            if (id) apply(id);
          }}
          options={[
            { value: "", label: "选择预设…" },
            ...visible.map((p) => ({
              value: p.id,
              label: `${p.name}${p.builtin ? "" : "（自定义）"}`,
            })),
          ]}
        />
        <Button
          disabled={!current}
          title={current ? "把当前作用域的设置存为预设" : "当前作用域还没有设置"}
          onClick={() => {
            const name = window.prompt("给这个预设起个名字", "我的预设");
            if (name && name.trim()) void saveCurrentAsPreset(name.trim());
          }}
        >
          另存为预设
        </Button>
        <Button variant="ghost" onClick={() => setManaging((v) => !v)}>
          {managing ? "结束管理" : "管理"}
        </Button>
      </div>

      {managing && (
        <ul className="mt-2 space-y-1 rounded-md border border-border bg-surface-2 p-2">
          {visible.map((preset) => (
            <li key={preset.id} className="flex items-center gap-2 text-[11px]">
              <span className="min-w-0 flex-1 truncate" title={preset.name}>
                {preset.name}
              </span>
              <Tag>{preset.builtin ? "内置" : "自定义"}</Tag>
              {!preset.builtin && (
                <button
                  type="button"
                  className="btn btn-ghost px-1.5 py-0"
                  onClick={() => {
                    const name = window.prompt("重命名预设", preset.name);
                    if (name && name.trim()) void renamePreset(preset.id, name.trim());
                  }}
                >
                  重命名
                </button>
              )}
              <button
                type="button"
                className="btn btn-ghost px-1.5 py-0"
                title={preset.builtin ? "内置预设无法删除，将改为隐藏" : "删除该预设"}
                onClick={() => void removePreset(preset.id)}
              >
                {preset.builtin ? "隐藏" : "删除"}
              </button>
            </li>
          ))}
        </ul>
      )}
    </Field>
  );
}
