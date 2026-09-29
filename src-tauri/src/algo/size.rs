use crate::error::{AppError, AppResult};
use crate::model::{Anchor, Computed, GroupSetting, Mode, Setting, SettingSource};

/// 尺寸上限（规范 6.2 / 附录 B 第 3 项）
pub const MAX_DIMENSION: u32 = 32768;

/// 向下取偶：`n => n - (n % 2)`（规范 10.1）
pub fn floor_even(n: u32) -> u32 {
    n - (n % 2)
}

/// 三级优先级解析（规范 10.1 / 6.4）。
///
/// ```text
/// 单文件设置  →  所属分组设置  →  整体设置  →  不做任何改动
/// ```
///
/// 同时返回生效设置的来源，供界面显示「生效设置来源」。
pub fn resolve_setting(
    file: &Option<Setting>,
    group: &Option<GroupSetting>,
    global: &Option<Setting>,
) -> (Option<Setting>, SettingSource) {
    if let Some(s) = file {
        return (Some(s.clone()), SettingSource::File);
    }

    match group {
        Some(GroupSetting::Explicit(s)) => (Some(s.clone()), SettingSource::Group),
        Some(GroupSetting::FollowGlobal) => match global {
            Some(g) => (Some(g.clone()), SettingSource::Global),
            None => (None, SettingSource::None),
        },
        None => match global {
            Some(g) => (Some(g.clone()), SettingSource::Global),
            None => (None, SettingSource::None),
        },
    }
}

/// 按模式求缩放后的浮点尺寸，以及画布尺寸（仅模式 B 补边时与内容尺寸不同）。
struct RawDims {
    content_w: f64,
    content_h: f64,
    canvas_w: f64,
    canvas_h: f64,
    anchor: Anchor,
}

fn raw_dims(orig_w: u32, orig_h: u32, setting: &Setting) -> AppResult<RawDims> {
    let ow = orig_w as f64;
    let oh = orig_h as f64;

    let (content_w, content_h, canvas_w, canvas_h, anchor) = match setting.mode {
        Mode::A | Mode::G => {
            let r = setting
                .scale
                .ok_or_else(|| AppError::new(crate::error::ErrorCode::IncompleteDimension, "缺少倍率"))?;
            let w = ow * r;
            let h = oh * r;
            (
                w,
                h,
                w,
                h,
                setting.anchor.unwrap_or(Anchor::Center),
            )
        }
        Mode::B => {
            let (tw, th) = target_box(setting)?;
            let r = (tw / ow).min(th / oh);
            let w = ow * r;
            let h = oh * r;
            if setting.no_pad {
                (w, h, w, h, Anchor::Center)
            } else {
                // 默认输出 目标框尺寸画布，等比内容居中，空余区域填充
                (w, h, tw, th, Anchor::Center)
            }
        }
        Mode::C => {
            let (tw, th) = target_box(setting)?;
            // 等比缩放至覆盖目标框后居中裁剪，输出即目标框
            (tw, th, tw, th, Anchor::Center)
        }
        Mode::D => {
            let (tw, th) = target_box(setting)?;
            (tw, th, tw, th, Anchor::Center)
        }
        Mode::E => {
            let limit = setting.limit.ok_or_else(|| {
                AppError::new(crate::error::ErrorCode::IncompleteDimension, "缺少长边上限")
            })?;
            let m = ow.max(oh);
            let r = limit / m;
            let w = ow * r;
            let h = oh * r;
            (w, h, w, h, Anchor::Center)
        }
    };

    Ok(RawDims {
        content_w,
        content_h,
        canvas_w,
        canvas_h,
        anchor,
    })
}

/// 模式 B / C / D 的目标框；关闭联动后只填一边时按 13.2 报 `E_INCOMPLETE_DIMENSION` 并指明缺哪边。
fn target_box(setting: &Setting) -> AppResult<(f64, f64)> {
    match (setting.width, setting.height) {
        (Some(w), Some(h)) => Ok((w, h)),
        (None, Some(_)) => Err(AppError::incomplete_dimension(true)),
        (Some(_), None) => Err(AppError::incomplete_dimension(false)),
        (None, None) => Err(AppError::incomplete_dimension(true)),
    }
}

/// 计算目标尺寸（规范 10.1）。
///
/// - `Ok(None)` 表示「未改动」，调用方不得写出、不得复制。
/// - `Ok(Some(_))` 表示需要缩放并写出。
/// - `Err(_)` 为校验失败，须在 `VALIDATING` 阶段拦截，不执行任何写出。
pub fn compute_target(
    orig_w: u32,
    orig_h: u32,
    setting: &Setting,
    is_video: bool,
) -> AppResult<Option<Computed>> {
    // 模式 E 在不超过上限时直接判定为未改动
    if setting.mode == Mode::E {
        if let Some(limit) = setting.limit {
            let m = (orig_w as f64).max(orig_h as f64);
            if m <= limit {
                return Ok(None);
            }
        }
    }

    let raw = raw_dims(orig_w, orig_h, setting)?;

    let round = |v: f64| -> u32 {
        let n = v.round();
        let n = if n < 0.0 { 0 } else { n as u32 };
        if is_video {
            floor_even(n)
        } else {
            n
        }
    };

    let w = round(raw.canvas_w);
    let h = round(raw.canvas_h);
    let mut cw = round(raw.content_w);
    let mut ch = round(raw.content_h);

    // 内容不得超出画布（浮点误差可能让内容比画布大 1px）
    cw = cw.min(w);
    ch = ch.min(h);

    // F 附加开关：仅缩小 / 仅放大。以画布尺寸与原尺寸比较。
    if setting.only_down && (w > orig_w || h > orig_h) {
        return Ok(None);
    }
    if setting.only_up && (w < orig_w || h < orig_h) {
        return Ok(None);
    }

    // 边界校验
    if w < 1 || h < 1 {
        return Err(AppError::size_underflow());
    }
    if w > MAX_DIMENSION || h > MAX_DIMENSION {
        return Err(AppError::size_overflow());
    }

    if w == orig_w && h == orig_h {
        return Ok(None);
    }

    Ok(Some(Computed {
        width: w,
        height: h,
        content_width: cw,
        content_height: ch,
        anchor: raw.anchor,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Setting;

    fn a(scale: f64) -> Setting {
        Setting::scale(Mode::A, scale)
    }

    /// 规范 10.1 尺寸计算用例表，12 行全部覆盖。
    #[test]
    fn spec_table_10_1() {
        // 1920×1080 | A | 倍率 0.5 | 960×540
        let c = compute_target(1920, 1080, &a(0.5), false).unwrap().unwrap();
        assert_eq!((c.width, c.height), (960, 540));

        // 1920×1080 | A | 倍率 3 | 5760×3240
        let c = compute_target(1920, 1080, &a(3.0), false).unwrap().unwrap();
        assert_eq!((c.width, c.height), (5760, 3240));

        // 1920×1080 | B | 1280×720 | 1280×720 画布，内容满幅
        let s = Setting::width_height(Mode::B, 1280.0, 720.0);
        let c = compute_target(1920, 1080, &s, false).unwrap().unwrap();
        assert_eq!((c.width, c.height), (1280, 720));
        assert_eq!((c.content_width, c.content_height), (1280, 720));

        // 1920×1080 | B | 800×800 | 800×800 画布，内容 800×450 居中
        let s = Setting::width_height(Mode::B, 800.0, 800.0);
        let c = compute_target(1920, 1080, &s, false).unwrap().unwrap();
        assert_eq!((c.width, c.height), (800, 800));
        assert_eq!((c.content_width, c.content_height), (800, 450));
        assert_eq!(c.anchor, Anchor::Center);

        // 1920×1080 | C | 800×800 | 800×800（缩放至 1422×800 后居中裁剪）
        let s = Setting::width_height(Mode::C, 800.0, 800.0);
        let c = compute_target(1920, 1080, &s, false).unwrap().unwrap();
        assert_eq!((c.width, c.height), (800, 800));

        // 1920×1080 | D | 800×800 | 800×800（变形）
        let s = Setting::width_height(Mode::D, 800.0, 800.0);
        let c = compute_target(1920, 1080, &s, false).unwrap().unwrap();
        assert_eq!((c.width, c.height), (800, 800));

        // 1920×1080 | E | 最大边 1280 | 1280×720
        let s = Setting {
            limit: Some(1280.0),
            ..Setting::new(Mode::E)
        };
        let c = compute_target(1920, 1080, &s, false).unwrap().unwrap();
        assert_eq!((c.width, c.height), (1280, 720));

        // 640×480 | E | 最大边 1280 | 不变
        let s = Setting {
            limit: Some(1280.0),
            ..Setting::new(Mode::E)
        };
        assert_eq!(compute_target(640, 480, &s, false).unwrap(), None);

        // 1921×1081 | A | 倍率 1 | 图片：不变；视频：1920×1080
        assert_eq!(compute_target(1921, 1081, &a(1.0), false).unwrap(), None);
        let c = compute_target(1921, 1081, &a(1.0), true).unwrap().unwrap();
        assert_eq!((c.width, c.height), (1920, 1080));

        // 100×100 | F 仅缩小 | 倍率 2 | 不变
        let s = Setting {
            only_down: true,
            ..a(2.0)
        };
        assert_eq!(compute_target(100, 100, &s, false).unwrap(), None);

        // 1920×1080 | A | 倍率 1 | 不变（标记为「未改动」）
        assert_eq!(compute_target(1920, 1080, &a(1.0), false).unwrap(), None);

        // 1920×1080 | A | 倍率 100 | 报错：超出尺寸上限
        let err = compute_target(1920, 1080, &a(100.0), false).unwrap_err();
        assert_eq!(err.code, crate::error::ErrorCode::SizeOverflow);
    }

    #[test]
    fn f_switch_only_up() {
        // 仅放大：目标比原图小则不动
        let s = Setting {
            only_up: true,
            ..a(0.5)
        };
        assert_eq!(compute_target(1920, 1080, &s, false).unwrap(), None);

        let s = Setting {
            only_up: true,
            ..a(2.0)
        };
        let c = compute_target(1920, 1080, &s, false).unwrap().unwrap();
        assert_eq!((c.width, c.height), (3840, 2160));
    }

    #[test]
    fn mode_b_no_pad_outputs_content_size() {
        let s = Setting {
            no_pad: true,
            ..Setting::width_height(Mode::B, 800.0, 800.0)
        };
        let c = compute_target(1920, 1080, &s, false).unwrap().unwrap();
        assert_eq!((c.width, c.height), (800, 450));
    }

    #[test]
    fn mode_g_keeps_anchor() {
        let s = Setting {
            anchor: Some(Anchor::TopLeft),
            ..Setting::scale(Mode::G, 0.5)
        };
        let c = compute_target(1920, 1080, &s, false).unwrap().unwrap();
        assert_eq!((c.width, c.height), (960, 540));
        assert_eq!(c.anchor, Anchor::TopLeft);
    }

    #[test]
    fn video_forces_even_both_axes() {
        let c = compute_target(1921, 1081, &a(1.0), true).unwrap().unwrap();
        assert_eq!((c.width, c.height), (1920, 1080));
        // 图片不强制偶数
        let c = compute_target(1921, 1081, &a(1.0), false);
        assert_eq!(c.unwrap(), None);
    }

    #[test]
    fn image_not_forced_even() {
        let c = compute_target(101, 101, &a(1.5), false).unwrap().unwrap();
        assert_eq!((c.width, c.height), (152, 152)); // 151.5 → 四舍五入 152
    }

    #[test]
    fn underflow_below_one_pixel() {
        // 10 × 0.01 = 0.1px → 四舍五入为 0，必须报错而不是静默产出 0 尺寸文件
        let err = compute_target(10, 10, &a(0.01), false).unwrap_err();
        assert_eq!(err.code, crate::error::ErrorCode::SizeUnderflow);
    }

    #[test]
    fn incomplete_dimension_names_missing_side() {
        let s = Setting {
            width: Some(800.0),
            ..Setting::new(Mode::D)
        };
        let err = compute_target(1920, 1080, &s, false).unwrap_err();
        assert_eq!(err.code, crate::error::ErrorCode::IncompleteDimension);
        assert!(err.message.contains("缺少高度"), "实际：{}", err.message);

        let s = Setting {
            height: Some(800.0),
            ..Setting::new(Mode::D)
        };
        let err = compute_target(1920, 1080, &s, false).unwrap_err();
        assert!(err.message.contains("缺少宽度"), "实际：{}", err.message);
    }

    #[test]
    fn resolve_setting_priority_and_tri_state() {
        let global = Some(a(2.0));
        let group_set = Some(GroupSetting::Explicit(a(0.5)));
        let file_set = Some(a(3.0));

        // 单文件 > 分组 > 整体
        let (s, src) = resolve_setting(&file_set, &group_set, &global);
        assert_eq!(s.unwrap().scale, Some(3.0));
        assert_eq!(src, SettingSource::File);

        // 无单文件设置 → 分组
        let (s, src) = resolve_setting(&None, &group_set, &global);
        assert_eq!(s.unwrap().scale, Some(0.5));
        assert_eq!(src, SettingSource::Group);

        // 分组显式跟随整体 → 取整体
        let follow = Some(GroupSetting::FollowGlobal);
        let (s, src) = resolve_setting(&None, &follow, &global);
        assert_eq!(s.unwrap().scale, Some(2.0));
        assert_eq!(src, SettingSource::Global);

        // 分组未设置 → 整体
        let (s, src) = resolve_setting(&None, &None, &global);
        assert_eq!(s.unwrap().scale, Some(2.0));
        assert_eq!(src, SettingSource::Global);

        // 三层全空 → 不做任何改动
        let (s, src) = resolve_setting(&None, &None, &None);
        assert!(s.is_none());
        assert_eq!(src, SettingSource::None);
    }

    #[test]
    fn follow_global_without_global_setting_is_unchanged() {
        let follow = Some(GroupSetting::FollowGlobal);
        let (s, src) = resolve_setting(&None, &follow, &None);
        assert!(s.is_none());
        assert_eq!(src, SettingSource::None);
    }

    #[test]
    fn floor_even_definition() {
        assert_eq!(floor_even(1921), 1920);
        assert_eq!(floor_even(1081), 1080);
        assert_eq!(floor_even(1920), 1920);
        assert_eq!(floor_even(1), 0);
    }
}
