import { useEffect, useState } from "react";

import { parseDimension, parseScale } from "../lib/expression";
import { referenceFor, useStore } from "../store";
import { ANCHOR_LABEL, MODES, MODE_LABEL, type Anchor, type Mode, type Setting } from "../types";
import ImageOptionsPanel from "./ImageOptionsPanel";
import VideoOptionsPanel from "./VideoOptionsPanel";
import { Button, Checkbox, EmptyHint, Field, Panel, Select, Tag, TextInput } from "./ui";

const ANCHORS: Anchor[] = [
  "topLeft",
  "top",
  "topRight",
  "left",
  "center",
  "right",
  "bottomLeft",
  "bottom",
  "bottomRight",
];

export default function SettingsPanel({
  className = "",
  width,
}: {
  className?: string;
  width?: number;
}) {
  const scope = useStore((s) => s.scope);
  const groups = useStore((s) => s.groups);
  const files = useStore((s) => s.files);
  const globalSetting = useStore((s) => s.globalSetting);
  const groupTiers = useStore((s) => s.groupTiers);
  const fileSettings = useStore((s) => s.fileSettings);
  const linkEnabled = useStore((s) => s.linkEnabled);
  const basis = useStore((s) => s.basis);
  const settings = useStore((s) => s.settings);

  const setGlobalSetting = useStore((s) => s.setGlobalSetting);
  const setGroupTier = useStore((s) => s.setGroupTier);
  const setFileSetting = useStore((s) => s.setFileSetting);
  const setLinkEnabled = useStore((s) => s.setLinkEnabled);
  const setBasis = useStore((s) => s.setBasis);
  const clearAllSettings = useStore((s) => s.clearAllSettings);

  const reference = referenceFor(useStore(), scope);

  const tier = scope.type === "group" ? (groupTiers[scope.name] ?? null) : null;
  const scopeSetting: Setting | null =
    scope.type === "global"
      ? globalSetting
      : scope.type === "group"
        ? tier?.kind === "explicit"
          ? tier.setting
          : null
        : (fileSettings[scope.id] ?? null);

  const [mode, setMode] = useState<Mode>(scopeSetting?.mode ?? "A");
  const [scaleText, setScaleText] = useState("");
  const [widthText, setWidthText] = useState("");
  const [heightText, setHeightText] = useState("");

  useEffect(() => {
    setMode(scopeSetting?.mode ?? "A");
    setScaleText(scopeSetting?.scale !== undefined ? String(scopeSetting.scale) : "");
    setWidthText(scopeSetting?.width !== undefined ? String(scopeSetting.width) : "");
    setHeightText(scopeSetting?.height !== undefined ? String(scopeSetting.height) : "");
  }, [scope, scopeSetting]);

  const apply = (next: Setting) => {
    if (scope.type === "global") setGlobalSetting(next);
    else if (scope.type === "group") setGroupTier(scope.name, { kind: "explicit", setting: next });
    else setFileSetting(scope.id, next);
  };

  const commit = (patch: Partial<Setting>) => apply({ ...(scopeSetting ?? { mode }), ...patch });

  const clearScope = () => {
    if (scope.type === "global") setGlobalSetting(null);
    else if (scope.type === "group") setGroupTier(scope.name, null);
    else setFileSetting(scope.id, null);
  };

  const groupName =
    scope.type === "file" ? groups.find((g) => g.fileIds.includes(scope.id))?.name : undefined;
  const scopeLabel =
    scope.type === "global"
      ? "整体"
      : scope.type === "group"
        ? `分组 ${scope.name}`
        : (files.find((f) => f.id === scope.id)?.relativePath ?? scope.id);

  const scaleResult = parseScale(scaleText);
  const widthResult = parseDimension(widthText);
  const heightResult = parseDimension(heightText);

  const effectiveScale = (() => {
    if (mode === "A" || mode === "G") return scaleResult.value;
    if (mode === "B" || mode === "C" || mode === "D") {
      if (widthResult.value !== null && reference.width > 0) return widthResult.value / reference.width;
      if (heightResult.value !== null && reference.height > 0)
        return heightResult.value / reference.height;
      return null;
    }
    if (mode === "E") {
      const limit = Number(scaleText);
      const longest = Math.max(reference.width, reference.height);
      return Number.isFinite(limit) && longest > 0 ? limit / longest : null;
    }
    return null;
  })();

  const threshold = settings?.processing?.upscaleWarnThreshold;
  const warnThreshold = typeof threshold === "number" ? threshold : 4;
  const upscaleWarning =
    effectiveScale !== null && effectiveScale > warnThreshold ? effectiveScale : null;

  if (files.length === 0) {
    return (
      <Panel title="参数设置区" className={className} width={width}>
        <div className="panel-body space-y-3">
          <EmptyHint>
            <span>载入素材后在此设置缩放参数</span>
          </EmptyHint>
          <ImageOptionsPanel />
          <VideoOptionsPanel />
        </div>
      </Panel>
    );
  }

  return (
    <Panel
      title="参数设置区"
      className={className}
      width={width}
      right={
        <>
          <Button variant="ghost" onClick={clearScope}>
            清除本层
          </Button>
          <Button variant="ghost" onClick={clearAllSettings}>
            全部清空
          </Button>
        </>
      }
    >
      <div className="panel-body space-y-3">
        <div className="flex flex-wrap items-center gap-1.5">
          <span
            className="max-w-full truncate rounded-md bg-surface-3 px-2 py-0.5 text-[11px] text-muted"
            title={scopeLabel}
          >
            {scopeLabel}
          </span>
          {scopeSetting && <Tag tone="accent">本层已设置</Tag>}
          {groupName && <Tag>属于 {groupName}</Tag>}
        </div>

        {/* 三态：未设置 / 跟随整体 / 已设置（规范 6.4） */}
        {scope.type === "group" && (
          <Field label="本组状态">
            <Select
              value={tier === null ? "unset" : tier.kind === "followGlobal" ? "follow" : "explicit"}
              onChange={(value) => {
                if (value === "unset") setGroupTier(scope.name, null);
                else if (value === "follow") setGroupTier(scope.name, { kind: "followGlobal" });
                else setGroupTier(scope.name, { kind: "explicit", setting: { mode } });
              }}
              options={[
                { value: "unset", label: "未设置（继承整体）" },
                { value: "follow", label: "跟随整体（显式）" },
                { value: "explicit", label: "已设置" },
              ]}
            />
          </Field>
        )}

        {/* 缩放方式 A–G 单选 */}
        <div className="space-y-1">
          <span className="text-[11px] text-muted">缩放方式</span>
          <div className="grid grid-cols-2 gap-1">
            {MODES.map((m) => (
              <button
                key={m}
                type="button"
                title={MODE_LABEL[m]}
                onClick={() => {
                  setMode(m);
                  apply({ ...(scopeSetting ?? { mode: m }), mode: m });
                }}
                className={`truncate rounded-md border px-2 py-1.5 text-left text-[11px] transition ${
                  mode === m
                    ? "border-accent bg-accent-soft text-accent"
                    : "border-border text-muted hover:border-border-strong hover:text-text"
                }`}
              >
                {MODE_LABEL[m]}
              </button>
            ))}
          </div>
        </div>

        {(mode === "A" || mode === "G") && (
          <Field
            label="缩放倍率"
            hint={
              reference.width > 0 ? (
                <>
                  基准 {reference.width}×{reference.height}
                  {scaleResult.value !== null && (
                    <>
                      {" → "}
                      <span className="font-mono text-muted">
                        {Math.round(reference.width * scaleResult.value)}×
                        {Math.round(reference.height * scaleResult.value)}
                      </span>
                    </>
                  )}
                </>
              ) : null
            }
          >
            <TextInput
              value={scaleText}
              invalid={scaleText !== "" && scaleResult.error !== null}
              placeholder="2、0.5、150%、1920/2"
              onChange={(text) => {
                setScaleText(text);
                const parsed = parseScale(text);
                if (parsed.value !== null) commit({ scale: parsed.value });
              }}
            />
          </Field>
        )}

        {mode === "E" && (
          <Field label="长边上限" hint="未超过上限的素材保持原样">
            <TextInput
              value={scaleText}
              placeholder="1280"
              onChange={(text) => {
                setScaleText(text);
                const parsed = parseDimension(text);
                if (parsed.value !== null) commit({ limit: parsed.value });
              }}
            />
          </Field>
        )}

        {(mode === "B" || mode === "C" || mode === "D") && (
          <div className="space-y-3">
            <div className="grid grid-cols-2 gap-2">
              <Field label="目标宽">
                <TextInput
                  value={widthText}
                  invalid={widthText !== "" && widthResult.error !== null}
                  placeholder="1920"
                  onChange={(text) => {
                    setWidthText(text);
                    const parsed = parseDimension(text);
                    if (parsed.value !== null) commit({ width: parsed.value });
                  }}
                />
              </Field>
              <Field label="目标高">
                <TextInput
                  value={heightText}
                  invalid={heightText !== "" && heightResult.error !== null}
                  placeholder="1080"
                  onChange={(text) => {
                    setHeightText(text);
                    const parsed = parseDimension(text);
                    if (parsed.value !== null) commit({ height: parsed.value });
                  }}
                />
              </Field>
            </div>

            <Checkbox checked={linkEnabled} onChange={setLinkEnabled}>
              按比例自动计算
            </Checkbox>

            {!linkEnabled && (
              <p className="text-[11px] text-warn">宽高各自独立，只填一边将报错并要求补全</p>
            )}

            <div className="flex items-center gap-2">
              <span className="shrink-0 text-[11px] text-faint">基准</span>
              <Select
                value={basis}
                onChange={setBasis}
                options={[
                  { value: "selection", label: "当前选中素材尺寸" },
                  { value: "groupMax", label: "分组内最大尺寸" },
                  { value: "groupMin", label: "分组内最小尺寸" },
                ]}
              />
              {widthResult.value !== null && reference.width > 0 && (
                <span className="shrink-0 text-[11px] text-faint">
                  等效 <span className="font-mono text-muted">
                    {(widthResult.value / reference.width).toFixed(3)}
                  </span>{" "}
                  倍
                </span>
              )}
            </div>

            {mode === "B" && (
              <Checkbox checked={scopeSetting?.noPad ?? false} onChange={(v) => commit({ noPad: v })}>
                不补边（输出内容实际尺寸）
              </Checkbox>
            )}
          </div>
        )}

        {mode === "G" && (
          <Field label="锚点">
            <Select
              value={scopeSetting?.anchor ?? "center"}
              onChange={(value) => commit({ anchor: value as Anchor })}
              options={ANCHORS.map((a) => ({ value: a, label: ANCHOR_LABEL[a] }))}
            />
          </Field>
        )}

        <div className="flex items-center gap-4 border-t border-border pt-3">
          <span className="text-[11px] text-faint">附加</span>
          <Checkbox checked={scopeSetting?.onlyUp ?? false} onChange={(v) => commit({ onlyUp: v })}>
            仅放大
          </Checkbox>
          <Checkbox checked={scopeSetting?.onlyDown ?? false} onChange={(v) => commit({ onlyDown: v })}>
            仅缩小
          </Checkbox>
        </div>

        {upscaleWarning !== null && (
          <div className="rounded-md border border-warn/40 bg-warn-soft px-2.5 py-2 text-[11px] leading-relaxed text-warn">
            放大 <span className="font-mono">{upscaleWarning.toFixed(2)}</span> 倍，已超过
            {warnThreshold} 倍阈值：放大不会增加画面细节，原素材分辨率不足时结果会模糊或出现锯齿。
          </div>
        )}

        <ImageOptionsPanel />
        <VideoOptionsPanel />
      </div>
    </Panel>
  );
}
