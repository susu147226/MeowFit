import { pickBackgroundImage } from "../api";
import { useStore } from "../store";
import { ACCENT_PRESETS, DENSITY_LABEL, THEME_MODE_LABEL, type Density, type ThemeMode } from "../types";
import { Button, Checkbox, Field, Tag } from "./ui";

const MODES: ThemeMode[] = ["system", "light", "dark"];
const DENSITIES: Density[] = ["compact", "standard", "relaxed"];

/** 外观设置：外观模式、配色、背景图、布局（规范 6.14 / 第七节）。 */
export default function AppearanceDialog() {
  const theme = useStore((s) => s.settings?.theme);
  const resolvedTheme = useStore((s) => s.resolvedTheme);
  const backgroundUrl = useStore((s) => s.backgroundUrl);
  const setTheme = useStore((s) => s.setTheme);
  const setAppearanceOpen = useStore((s) => s.setAppearanceOpen);

  if (!theme) return null;

  // 背景图透明度高且未加蒙层时，前景文字可能不易辨认（规范 6.14）
  const contrastRisk = backgroundUrl !== null && !theme.autoScrim && theme.backgroundOpacity > 40;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-6"
      onClick={() => setAppearanceOpen(false)}
    >
      <div
        className="max-h-full w-[480px] overflow-auto rounded-lg border border-border bg-surface p-5 shadow-xl"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center gap-2">
          <h2 className="text-[15px] font-semibold">外观</h2>
          {theme.mode === "system" && <Tag>当前跟随系统 · {resolvedTheme === "dark" ? "深色" : "浅色"}</Tag>}
        </div>

        <div className="mt-4 space-y-4">
          <Field label="外观模式" hint="默认跟随系统，系统切换深浅色时会自动跟随">
            <div className="flex gap-1">
              {MODES.map((mode) => (
                <button
                  key={mode}
                  type="button"
                  onClick={() => void setTheme({ mode })}
                  className={`flex-1 rounded-md border px-2 py-1.5 text-[12px] transition ${
                    theme.mode === mode
                      ? "border-accent bg-accent-soft text-accent"
                      : "border-border text-muted hover:border-border-strong hover:text-text"
                  }`}
                >
                  {THEME_MODE_LABEL[mode]}
                </button>
              ))}
            </div>
          </Field>

          <Field label="主题色" hint="界面强调色，深浅两套会各自校正以保证可读性">
            <div className="flex flex-wrap items-center gap-1.5">
              {ACCENT_PRESETS.map((preset) => (
                <button
                  key={preset.value}
                  type="button"
                  title={preset.name}
                  onClick={() => void setTheme({ accent: preset.value })}
                  style={{ backgroundColor: preset.value }}
                  className={`h-6 w-6 rounded-md border transition ${
                    theme.accent.toUpperCase() === preset.value.toUpperCase()
                      ? "border-text ring-2 ring-accent/40"
                      : "border-border"
                  }`}
                />
              ))}
              <input
                type="color"
                title="自定义主题色"
                value={theme.accent}
                onChange={(e) => void setTheme({ accent: e.target.value.toUpperCase() })}
                className="h-6 w-8 cursor-pointer rounded-md border border-border bg-surface"
              />
            </div>
          </Field>

          <Field label="界面密度" hint="影响各面板与表格行的内边距">
            <div className="flex gap-1">
              {DENSITIES.map((density) => (
                <button
                  key={density}
                  type="button"
                  onClick={() => void setTheme({ density })}
                  className={`flex-1 rounded-md border px-2 py-1.5 text-[12px] transition ${
                    theme.density === density
                      ? "border-accent bg-accent-soft text-accent"
                      : "border-border text-muted hover:border-border-strong hover:text-text"
                  }`}
                >
                  {DENSITY_LABEL[density]}
                </button>
              ))}
            </div>
          </Field>

          <Field label="界面背景图" hint="仅用于界面外观，不进入素材处理流程，也不会上传">
            <div className="flex items-center gap-2">
              <Button
                onClick={() => {
                  void pickBackgroundImage().then((path) => {
                    if (path) void setTheme({ backgroundImage: path });
                  });
                }}
              >
                {theme.backgroundImage ? "更换图片" : "选择图片"}
              </Button>
              {theme.backgroundImage && (
                <Button variant="ghost" onClick={() => void setTheme({ backgroundImage: null })}>
                  移除
                </Button>
              )}
              {theme.backgroundImage && (
                <span
                  className="min-w-0 flex-1 truncate font-mono text-[11px] text-faint"
                  title={theme.backgroundImage}
                >
                  {theme.backgroundImage}
                </span>
              )}
            </div>
          </Field>

          {theme.backgroundImage && (
            <>
              <Field label={`背景不透明度 ${theme.backgroundOpacity}%`}>
                <input
                  type="range"
                  min={0}
                  max={100}
                  value={theme.backgroundOpacity}
                  onChange={(e) => void setTheme({ backgroundOpacity: Number(e.target.value) })}
                  className="w-full accent-accent"
                />
              </Field>

              <Checkbox checked={theme.autoScrim} onChange={(v) => void setTheme({ autoScrim: v })}>
                自动加蒙层（背景较亮时保证文字可读）
              </Checkbox>

              {contrastRisk && (
                <div className="rounded-md border border-warn/40 bg-warn-soft px-2.5 py-2 text-[11px] leading-relaxed text-warn">
                  背景图较明显且未加蒙层，前景文字可能不易辨认。建议开启「自动加蒙层」，或调低不透明度。
                </div>
              )}
            </>
          )}

          <Field label="界面布局" hint="可直接拖动主界面中两条竖向分隔条调整各列宽度">
            <div className="flex items-center gap-2">
              <span className="font-mono text-[11px] text-faint">
                参数列 {theme.sidebarWidth}px · 预览列 {theme.previewWidth}px
              </span>
              <Button onClick={() => void setTheme({ sidebarWidth: 350, previewWidth: 350 })}>
                恢复默认列宽
              </Button>
            </div>
          </Field>
        </div>

        <div className="mt-5 flex justify-between">
          <Button
            onClick={() =>
              void setTheme({
                mode: "system",
                accent: ACCENT_PRESETS[0].value,
                density: "standard",
                backgroundImage: null,
                backgroundOpacity: 100,
                autoScrim: true,
                sidebarWidth: 350,
                previewWidth: 350,
              })
            }
          >
            恢复默认外观
          </Button>
          <Button variant="primary" onClick={() => setAppearanceOpen(false)}>
            完成
          </Button>
        </div>
      </div>
    </div>
  );
}
