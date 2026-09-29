import { useEffect, useState } from "react";

import { parseDimension, parseScale } from "../lib/expression";
import { referenceFor, useStore } from "../store";
import { ANCHOR_LABEL, MODES, MODE_LABEL, type Anchor, type Mode, type Setting } from "../types";
import { Badge, Button, EmptyHint, Field, RegionTitle, TextInput } from "./ui";

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

export default function SettingsPanel() {
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

  const state = useStore();
  const reference = referenceFor(state, scope);

  // 当前作用域下已生效的具体设置（不含「未设置 / 跟随整体」两种三态）
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

  // 切换作用域或外部设置变化时，同步输入框显示
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

  const base = (): Setting => scopeSetting ?? { mode };

  const commit = (patch: Partial<Setting>) => apply({ ...base(), ...patch });

  const clearScope = () => {
    if (scope.type === "global") setGlobalSetting(null);
    else if (scope.type === "group") setGroupTier(scope.name, null);
    else setFileSetting(scope.id, null);
  };

  const groupName = scope.type === "file" ? groups.find((g) => g.fileIds.includes(scope.id))?.name : undefined;
  const scopeLabel =
    scope.type === "global"
      ? "整体"
      : scope.type === "group"
        ? `分组：${scope.name}`
        : `单文件：${files.find((f) => f.id === scope.id)?.relativePath ?? scope.id}`;

  // 当前参数相对基准尺寸的等效倍率，用于放大阈值警告
  const scaleResult = parseScale(scaleText);
  const widthResult = parseDimension(widthText);
  const heightResult = parseDimension(heightText);
  const effectiveScale = (() => {
    if (!scopeSetting && !scaleText && !widthText && !heightText) return null;
    if (mode === "A" || mode === "G") return scaleResult.value;
    if (mode === "B" || mode === "C" || mode === "D") {
      if (widthResult.value !== null && reference.width > 0) return widthResult.value / reference.width;
      if (heightResult.value !== null && reference.height > 0) return heightResult.value / reference.height;
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

  const canEdit = scope.type === "global" || scope.type === "group" || scope.type === "file";

  if (files.length === 0) {
    return (
      <div className="flex min-h-0 flex-col">
        <RegionTitle>参数设置区</RegionTitle>
        <EmptyHint>载入素材后在此设置缩放参数。</EmptyHint>
      </div>
    );
  }

  return (
    <div className="flex min-h-0 flex-col border-b border-border">
      <RegionTitle
        right={
          <Button variant="ghost" onClick={clearScope} disabled={!canEdit}>
            清除本层设置
          </Button>
        }
      >
        参数设置区
      </RegionTitle>

      <div className="min-h-0 flex-1 space-y-2.5 overflow-auto px-3 py-2.5">
        <div className="flex flex-wrap items-center gap-2">
          <span className="rounded bg-surface-3 px-2 py-0.5 text-[11px] text-muted">{scopeLabel}</span>
          {scopeSetting && <Badge tone="accent">本层已设置</Badge>}
          {scope.type === "file" && groupName && <Badge>属于分组「{groupName}」</Badge>}
          <Button variant="ghost" onClick={clearAllSettings}>
            清空全部设置
          </Button>
        </div>

        {/* 三态：未设置 / 跟随整体 / 已设置（规范 6.4） */}
        {scope.type === "group" && (
          <div className="flex items-center gap-1.5">
            <span className="text-[11px] text-faint">本组状态</span>
            <select
              className="rounded border border-border bg-surface px-1.5 py-0.5 text-[11px]"
              value={tier === null ? "unset" : tier.kind === "followGlobal" ? "follow" : "explicit"}
              onChange={(e) => {
                const value = e.target.value;
                if (value === "unset") setGroupTier(scope.name, null);
                else if (value === "follow") setGroupTier(scope.name, { kind: "followGlobal" });
                else setGroupTier(scope.name, { kind: "explicit", setting: base() });
              }}
            >
              <option value="unset">未设置（继承整体）</option>
              <option value="follow">跟随整体（显式）</option>
              <option value="explicit">已设置</option>
            </select>
          </div>
        )}

        {/* 缩放方式 A–G 单选 */}
        <div className="space-y-1">
          <span className="text-[11px] text-faint">缩放方式</span>
          <div className="grid grid-cols-2 gap-1">
            {MODES.map((m) => (
              <button
                key={m}
                type="button"
                title={MODE_LABEL[m]}
                onClick={() => {
                  setMode(m);
                  apply({ ...base(), mode: m });
                }}
                className={`rounded border px-2 py-1 text-left text-[11px] transition ${
                  mode === m
                    ? "border-accent bg-accent-soft text-accent"
                    : "border-border text-muted hover:border-border-strong hover:text-text"
                }`}
              >
                {MODE_LABEL[m]}
              </button>
            ))}
          </div>
          <p className="text-[11px] text-faint">F「仅放大 / 仅缩小」是下方的附加开关，不是独立方式。</p>
        </div>

        {/* 参数输入 */}
        {(mode === "A" || mode === "G") && (
          <Field
            label="缩放倍率"
            hint={
              reference.width > 0 ? (
                <>
                  基准 {reference.width}×{reference.height}
                  {scaleResult.value !== null && (
                    <>
                      {" → 等效宽高 "}
                      <span className="font-mono text-muted">
                        {Math.round(reference.width * scaleResult.value)}×
                        {Math.round(reference.height * scaleResult.value)}
                      </span>
                    </>
                  )}
                </>
              ) : (
                "尚无可用的基准素材"
              )
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
          <Field label="长边上限（仅超过时等比缩小）" hint="未超过上限的素材保持原样">
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
          <div className="space-y-2">
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

            <label className="flex items-center gap-1.5 text-[12px] text-muted">
              <input
                type="checkbox"
                checked={linkEnabled}
                onChange={(e) => setLinkEnabled(e.target.checked)}
              />
              按比例自动计算
            </label>

            {linkEnabled ? (
              <p className="text-[11px] text-faint">
                只填一边即可，另一边按基准尺寸的宽高比自动算出
                {reference.width > 0 && (
                  <>
                    {"（基准 "}
                    {reference.width}×{reference.height}
                    {"）"}
                  </>
                )}
              </p>
            ) : (
              <p className="text-[11px] text-warn">
                已关闭联动：宽高各自独立，只填一边将报错并要求补全（缺少哪一边会明确指出）。
              </p>
            )}

            <div className="flex items-center gap-2">
              <span className="text-[11px] text-faint">基准</span>
              <select
                className="rounded border border-border bg-surface px-1.5 py-0.5 text-[11px]"
                value={basis}
                onChange={(e) => setBasis(e.target.value as typeof basis)}
              >
                <option value="selection">当前选中素材尺寸</option>
                <option value="groupMax">分组内最大尺寸</option>
                <option value="groupMin">分组内最小尺寸</option>
              </select>
              {widthResult.value !== null && reference.width > 0 && (
                <span className="text-[11px] text-faint">
                  等效倍率{" "}
                  <span className="font-mono text-muted">
                    {(widthResult.value / reference.width).toFixed(3)}
                  </span>
                </span>
              )}
            </div>

            {mode === "B" && (
              <label className="flex items-center gap-1.5 text-[12px] text-muted">
                <input
                  type="checkbox"
                  checked={scopeSetting?.noPad ?? false}
                  onChange={(e) => commit({ noPad: e.target.checked })}
                />
                不补边（输出内容实际尺寸，不输出目标框画布）
              </label>
            )}
          </div>
        )}

        {mode === "G" && (
          <div className="flex items-center gap-2">
            <span className="text-[11px] text-faint">锚点</span>
            <select
              className="rounded border border-border bg-surface px-1.5 py-0.5 text-[11px]"
              value={scopeSetting?.anchor ?? "center"}
              onChange={(e) => commit({ anchor: e.target.value as Anchor })}
            >
              {ANCHORS.map((a) => (
                <option key={a} value={a}>
                  {ANCHOR_LABEL[a]}
                </option>
              ))}
            </select>
          </div>
        )}

        {/* F 附加开关 */}
        <div className="flex items-center gap-4 border-t border-border pt-2">
          <span className="text-[11px] text-faint">F 附加</span>
          <label className="flex items-center gap-1.5 text-[12px] text-muted">
            <input
              type="checkbox"
              checked={scopeSetting?.onlyUp ?? false}
              onChange={(e) => commit({ onlyUp: e.target.checked })}
            />
            仅放大
          </label>
          <label className="flex items-center gap-1.5 text-[12px] text-muted">
            <input
              type="checkbox"
              checked={scopeSetting?.onlyDown ?? false}
              onChange={(e) => commit({ onlyDown: e.target.checked })}
            />
            仅缩小
          </label>
        </div>

        {upscaleWarning !== null && (
          <div className="rounded border border-warn/40 bg-warn-soft px-2 py-1.5 text-[11px] text-warn">
            放大倍率约 <span className="font-mono">{upscaleWarning.toFixed(2)}</span> 倍，已超过
            {warnThreshold} 倍阈值：放大不会增加画面细节，原作分辨率不足时结果会变模糊或出现锯齿。
            <span className="text-faint">（仅提示，不阻止执行）</span>
          </div>
        )}

        {/* 规范 6.2 要求界面说明放大行为 */}
        <p className="border-t border-border pt-2 text-[11px] leading-relaxed text-faint">
          放大不会增加画面细节。原素材分辨率不足时，放大结果会出现模糊、锯齿或马赛克。
          像素风素材应选 Nearest，普通素材应选 Lanczos3。（重采样算法选择与更细的画质控制在 P2 阶段接入。）
        </p>
      </div>
    </div>
  );
}
