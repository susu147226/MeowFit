use std::collections::HashMap;

use crate::algo::size::{compute_target, resolve_setting};
use crate::model::{
    Action, Plan, PlanEntry, PlanRequest, SettingSource,
};

/// 构建任务计划（规范 10.4）。
///
/// 前端预览与后端执行**共用**本函数，不存在第二份尺寸计算实现。
///
/// - `action == Unchanged` 的素材不写出、不复制、不转码，仅计入「未改动」。
/// - 任一素材计算报错则 `ok == false`，调用方须进入 `VALIDATION_FAILED` 且**不执行任何写出**。
pub fn build_plan(req: &PlanRequest) -> Plan {
    let group_settings: HashMap<&str, &Option<crate::model::GroupSetting>> = req
        .groups
        .iter()
        .map(|g| (g.name.as_str(), &g.setting))
        .collect();

    let mut entries: Vec<PlanEntry> = Vec::with_capacity(req.files.len());
    let mut ok = true;

    for file in &req.files {
        let group_setting = group_settings
            .get(file.group.as_str())
            .and_then(|s| (*s).clone());

        let (setting, source) = resolve_setting(&file.setting, &group_setting, &req.global);

        let setting = match setting {
            None => {
                // 三层均未设置：完全不做改动
                entries.push(base_entry(file, SettingSource::None, Action::Unchanged));
                continue;
            }
            Some(s) => s,
        };

        match compute_target(file.width, file.height, &setting, file.is_video) {
            Ok(None) => {
                entries.push(base_entry(file, source, Action::Unchanged));
            }
            Ok(Some(target)) => {
                entries.push(PlanEntry {
                    target: Some(target),
                    mode: Some(setting.mode),
                    ..base_entry(file, source, Action::Resize)
                });
            }
            Err(error) => {
                ok = false;
                entries.push(PlanEntry {
                    error: Some(error),
                    ..base_entry(file, source, Action::Unchanged)
                });
            }
        }
    }

    Plan { entries, ok }
}

fn base_entry(
    file: &crate::model::PlanFileInput,
    source: SettingSource,
    action: Action,
) -> PlanEntry {
    PlanEntry {
        id: file.id.clone(),
        name: file.name.clone(),
        group: file.group.clone(),
        source,
        action,
        original_width: file.width,
        original_height: file.height,
        target: None,
        mode: None,
        error: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{GroupSetting, Mode, PlanFileInput, PlanGroupInput, Setting};

    fn file(id: &str, group: &str, w: u32, h: u32) -> PlanFileInput {
        PlanFileInput {
            id: id.into(),
            name: format!("{id}.png"),
            width: w,
            height: h,
            is_video: false,
            group: group.into(),
            setting: None,
        }
    }

    fn find<'a>(plan: &'a Plan, id: &str) -> &'a PlanEntry {
        plan.entries.iter().find(|e| e.id == id).unwrap()
    }

    #[test]
    fn unset_files_stay_untouched() {
        // 场景 2 的核心行为：只为 icon 组设尺寸时，bg 组原地不动
        let req = PlanRequest {
            files: vec![file("a", "icon", 100, 100), file("b", "bg", 100, 100)],
            groups: vec![PlanGroupInput {
                name: "icon".into(),
                setting: Some(GroupSetting::Explicit(Setting::width_height(
                    Mode::D, 128.0, 128.0,
                ))),
            }],
            global: None,
        };

        let plan = build_plan(&req);
        assert!(plan.ok);
        assert_eq!(find(&plan, "a").action, Action::Resize);
        assert_eq!(find(&plan, "b").action, Action::Unchanged);
        assert_eq!(find(&plan, "b").source, SettingSource::None);
    }

    #[test]
    fn file_setting_beats_group_setting() {
        // 场景 3：单文件 256 覆盖分组 128
        let mut single = file("a", "icon", 100, 100);
        single.setting = Some(Setting::width_height(Mode::D, 256.0, 256.0));

        let req = PlanRequest {
            files: vec![single, file("b", "icon", 100, 100)],
            groups: vec![PlanGroupInput {
                name: "icon".into(),
                setting: Some(GroupSetting::Explicit(Setting::width_height(
                    Mode::D, 128.0, 128.0,
                ))),
            }],
            global: None,
        };

        let plan = build_plan(&req);
        assert_eq!(find(&plan, "a").target.unwrap().width, 256);
        assert_eq!(find(&plan, "a").source, SettingSource::File);
        assert_eq!(find(&plan, "b").target.unwrap().width, 128);
        assert_eq!(find(&plan, "b").source, SettingSource::Group);
    }

    #[test]
    fn group_setting_beats_global() {
        // 场景 4：整体 2 倍、bg 组 0.5 倍
        let req = PlanRequest {
            files: vec![file("a", "icon", 100, 100), file("b", "bg", 100, 100)],
            groups: vec![PlanGroupInput {
                name: "bg".into(),
                setting: Some(GroupSetting::Explicit(Setting::scale(Mode::A, 0.5))),
            }],
            global: Some(Setting::scale(Mode::A, 2.0)),
        };

        let plan = build_plan(&req);
        assert_eq!(find(&plan, "a").target.unwrap().width, 200);
        assert_eq!(find(&plan, "a").source, SettingSource::Global);
        assert_eq!(find(&plan, "b").target.unwrap().width, 50);
        assert_eq!(find(&plan, "b").source, SettingSource::Group);
    }

    #[test]
    fn follow_global_group_uses_global() {
        let req = PlanRequest {
            files: vec![file("a", "icon", 100, 100)],
            groups: vec![PlanGroupInput {
                name: "icon".into(),
                setting: Some(GroupSetting::FollowGlobal),
            }],
            global: Some(Setting::scale(Mode::A, 3.0)),
        };
        let plan = build_plan(&req);
        assert_eq!(find(&plan, "a").target.unwrap().width, 300);
        assert_eq!(find(&plan, "a").source, SettingSource::Global);
    }

    #[test]
    fn validation_error_marks_plan_not_ok_and_writes_nothing() {
        let mut bad = file("bad", "icon", 100, 100);
        bad.setting = Some(Setting::scale(Mode::A, 1000.0));

        let req = PlanRequest {
            files: vec![bad, file("ok", "icon", 100, 100)],
            groups: vec![PlanGroupInput {
                name: "icon".into(),
                setting: Some(GroupSetting::Explicit(Setting::width_height(
                    Mode::D, 128.0, 128.0,
                ))),
            }],
            global: None,
        };

        let plan = build_plan(&req);
        assert!(!plan.ok, "存在校验错误时 ok 必须为 false");
        let e = find(&plan, "bad");
        assert!(e.error.is_some());
        assert_eq!(
            e.error.as_ref().unwrap().code,
            crate::error::ErrorCode::SizeOverflow
        );
        assert_eq!(e.action, Action::Unchanged);
    }

    #[test]
    fn same_size_result_counts_as_unchanged() {
        let req = PlanRequest {
            files: vec![file("a", "g", 128, 128)],
            groups: vec![PlanGroupInput {
                name: "g".into(),
                setting: Some(GroupSetting::Explicit(Setting::width_height(
                    Mode::D, 128.0, 128.0,
                ))),
            }],
            global: None,
        };
        let plan = build_plan(&req);
        assert_eq!(find(&plan, "a").action, Action::Unchanged);
        assert!(find(&plan, "a").target.is_none());
    }

    #[test]
    fn video_gets_even_dimensions_in_plan() {
        let mut v = file("v", "g", 1921, 1081);
        v.is_video = true;
        let req = PlanRequest {
            files: vec![v],
            groups: vec![],
            global: Some(Setting::scale(Mode::A, 1.0)),
        };
        let plan = build_plan(&req);
        let t = find(&plan, "v").target.unwrap();
        assert_eq!((t.width, t.height), (1920, 1080));
    }
}
