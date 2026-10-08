import { useEffect, useRef, useState } from "react";

import { parseDimension, parseScale } from "../lib/expression";
import { referenceFor } from "../lib/planInput";
import { useStore } from "../store";
import { ANCHOR_LABEL, MODES, MODE_LABEL, type Anchor, type MediaKind, type Mode, type Setting } from "../types";
import AnimationOptionsPanel from "./AnimationOptionsPanel";
import ImageOptionsPanel from "./ImageOptionsPanel";
import OutputSettingsPanel from "./OutputSettingsPanel";
import PresetBar from "./PresetBar";
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

/**
 * 切换缩放方式时，只带走新方式用得上的参数。
 *
 * 带着上一种方式的不完整参数会让新方式一上来就是非法状态（例如从只填了宽度的
 * 模式 B 切到模式 A，宽度留着但缺倍率），于是报错一直挂着不消失。
 */
function settingForMode(next: Mode, prev: Setting | null): Setting {
  const out: Setting = { mode: next };
  if (!prev) return out;

  if (prev.onlyUp) out.onlyUp = true;
  if (prev.onlyDown) out.onlyDown = true;

  if (next === "A" || next === "G") {
    if (prev.scale !== undefined) {
      out.scale = prev.scale;
    } else if (prev.width !== undefined || prev.height !== undefined) {
      // 从「自定义宽高」切过来：把宽高折算成等效倍率，保留用户的意图
      out.width = prev.width;
      out.height = prev.height;
    }
    if (next === "G") out.anchor = prev.anchor ?? "center";
  } else if (next === "B" || next === "C" || next === "D") {
    if (prev.width !== undefined) out.width = prev.width;
    if (prev.height !== undefined) out.height = prev.height;
    if (next === "B" && prev.noPad) out.noPad = true;
  } else if (next === "E") {
    if (prev.limit !== undefined) out.limit = prev.limit;
  }
  return out;
}

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
  const setScope = useStore((s) => s.setScope);
  const selectedIds = useStore((s) => s.selectedIds);
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

  // 当前作用域涉及的素材类型：据此只显示相关的处理面板，减少冗余
  const scopeKinds = new Set<MediaKind>();
  if (scope.type === "global") {
    for (const f of files) if (f.kind) scopeKinds.add(f.kind);
  } else if (scope.type === "group") {
    const ids = new Set(groups.find((g) => g.name === scope.name)?.fileIds ?? []);
    for (const f of files) if (ids.has(f.id) && f.kind) scopeKinds.add(f.kind);
  } else {
    const f = files.find((f) => f.id === scope.id);
    if (f?.kind) scopeKinds.add(f.kind);
  }
  const showImage = scopeKinds.has("raster") || scopeKinds.has("svg");
  const showVideo = scopeKinds.has("video");
  const showAnimation = scopeKinds.has("animated");

  const [mode, setMode] = useState<Mode>(scopeSetting?.mode ?? "A");
  const [scaleText, setScaleText] = useState("");
  const [widthText, setWidthText] = useState("");
  const [heightText, setHeightText] = useState("");
  // 标记「这次作用域设置的变化是用户自己输入提交的」，回填 effect 据此跳过，
  // 避免把用户正在输入的内容（如「1.」）用已提交的数值覆盖掉。
  const selfCommittedRef = useRef(false);

  // 只在作用域「外部」发生变化时回填输入框。用户自己输入提交时跳过——
  // 否则输入「1.」会被当作 1 提交、随即被回写覆盖，小数点被吞，大于 1 的小数打不出来。
  useEffect(() => {
    if (selfCommittedRef.current) return;
    setMode(scopeSetting?.mode ?? "A");
    setScaleText(scopeSetting?.scale !== undefined ? String(scopeSetting.scale) : "");
    setWidthText(scopeSetting?.width !== undefined ? String(scopeSetting.width) : "");
    setHeightText(scopeSetting?.height !== undefined ? String(scopeSetting.height) : "");
  }, [scope, scopeSetting]);

  // 回填 effect 跑完后重置标记，让下一次外部变化（预设 / 清除 / 切换作用域）能正常回填
  useEffect(() => {
    selfCommittedRef.current = false;
  }, [scope, scopeSetting]);

  const apply = (next: Setting) => {
    if (scope.type === "global") setGlobalSetting(next);
    else if (scope.type === "group") setGroupTier(scope.name, { kind: "explicit", setting: next });
    else setFileSetting(scope.id, next);
  };

  const commit = (patch: Partial<Setting>) => {
    selfCommittedRef.current = true;
    apply({ ...(scopeSetting ?? { mode }), ...patch });
  };

  /** 模式 A/G 里填目标宽高时，按基准折算成等效倍率（规范 6.3 的两种输入）。 */
  const applyEquivalentScale = (width: number | null, height: number | null) => {
    if (reference.width <= 0 || reference.height <= 0) return;
    const candidates: number[] = [];
    if (width !== null) candidates.push(width / reference.width);
    if (height !== null) candidates.push(height / reference.height);
    if (candidates.length === 0) return;
    const scale = Math.min(...candidates);
    commit({ scale });
    setScaleText(String(Number(scale.toFixed(4))));
  };

  const clearScope = () => {
    if (scope.type === "global") setGlobalSetting(null);
    else if (scope.type === "group") setGroupTier(scope.name, null);
    else setFileSetting(scope.id, null);
  };

  const groupName =
    scope.type === "file" ? groups.find((g) => g.fileIds.includes(scope.id))?.name : undefined;
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
      <Panel title="参数设置区"
      dataPanelId="settings" className={className} width={width}>
        <div className="panel-body space-y-3">
          <EmptyHint>
            <span>载入素材后在此设置缩放参数</span>
          </EmptyHint>
          <ImageOptionsPanel />
          <VideoOptionsPanel />
          <AnimationOptionsPanel />
          <OutputSettingsPanel />
        </div>
      </Panel>
    );
  }

  return (
    <Panel
      title="参数设置区"
      dataPanelId="settings"
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
        {/* 作用域：一眼可选「整体 / 某分组 / 某文件」，避免改了某个分组却以为改了整体 */}
        <Field
          label="作用域"
          hint="设置只作用于所选作用域：单文件 > 分组 > 整体，未设置的层会向下继承"
        >
          <Select
            value={
              scope.type === "global"
                ? "global"
                : scope.type === "group"
                  ? `group:${scope.name}`
                  : `file:${scope.id}`
            }
            onChange={(value) => {
              if (value === "global") setScope({ type: "global" });
              else if (value.startsWith("group:")) setScope({ type: "group", name: value.slice(6) });
              else setScope({ type: "file", id: value.slice(5) });
            }}
            options={[
              { value: "global", label: `整体（全部 ${files.length} 个素材）` },
              ...groups.map((g) => ({
                value: `group:${g.name}`,
                label: `分组 ${g.name}（${g.fileIds.length} 个）`,
              })),
              ...selectedIds.map((id) => ({
                value: `file:${id}`,
                label: `单文件 ${files.find((f) => f.id === id)?.relativePath ?? id}`,
              })),
            ]}
          />
        </Field>

        <div className="flex flex-wrap items-center gap-1.5">
          {scopeSetting && <Tag tone="accent">本层已设置</Tag>}
          {scope.type === "group" && tier?.kind === "followGlobal" && <Tag>跟随整体</Tag>}
          {groupName && scope.type === "file" && <Tag>属于 {groupName}</Tag>}
          {scope.type !== "global" && !scopeSetting && <Tag>未设置，继承上一层</Tag>}
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

        <PresetBar current={scopeSetting} />

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
                  apply(settingForMode(m, scopeSetting));
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
          <div className="space-y-2">
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

            {/* 规范 6.3：每个可设置位置都支持「倍率」与「自定义宽高」两种输入 */}
            <div className="grid grid-cols-2 gap-2">
              <Field label="或填目标宽">
                <TextInput
                  value={widthText}
                  placeholder="1920"
                  onChange={(text) => {
                    setWidthText(text);
                    applyEquivalentScale(parseDimension(text).value, heightResult.value);
                  }}
                />
              </Field>
              <Field label="或填目标高">
                <TextInput
                  value={heightText}
                  placeholder="1080"
                  onChange={(text) => {
                    setHeightText(text);
                    applyEquivalentScale(widthResult.value, parseDimension(text).value);
                  }}
                />
              </Field>
            </div>
            <p className="text-[11px] leading-relaxed text-faint">
              填宽或高会按基准折算出等效倍率；两边都填时取能同时装下两者的倍率。等比缩放不补边。
            </p>
          </div>
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

        {showImage && <ImageOptionsPanel />}
        {showVideo && <VideoOptionsPanel />}
        {showAnimation && <AnimationOptionsPanel />}
        <OutputSettingsPanel />
      </div>
    </Panel>
  );
}
