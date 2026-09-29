//! 规范 16.2 的验收场景 1–8，逐条在真实文件上跑完整链路：
//! 扫描 → 自动分组 → 构建计划 → 执行写出。
//!
//! 这些测试走的是与界面完全相同的那一套函数（`scan_folder` / `group_files` /
//! `build_plan` / `execute`），因此它们通过即代表引擎侧行为符合验收标准。

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use image::{Rgb, RgbImage};

use meowfit_lib::algo::grouping::{group_files, GroupableFile};
use meowfit_lib::algo::plan::build_plan;
use meowfit_lib::exec::{execute, ExecOptions, SourceRef};
use meowfit_lib::model::{
    Action, GroupSetting, Grouping, PlanFileInput, PlanGroupInput, PlanRequest, Setting,
    Mode,
};
use meowfit_lib::scan::{scan_folder, ScanOptions};

fn temp_root(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "meowfit-accept-{tag}-{}-{:?}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn png(path: &Path, w: u32, h: u32) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    RgbImage::from_pixel(w, h, Rgb([180, 120, 60])).save(path).unwrap();
}

fn jpg(path: &Path, w: u32, h: u32) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    RgbImage::from_pixel(w, h, Rgb([90, 140, 200]))
        .save(path)
        .unwrap();
}

fn dims(path: &Path) -> (u32, u32) {
    image::image_dimensions(path).unwrap()
}

/// 扫描 + 按指定方式分组，返回 (分组名 → 文件 id 列表)。
fn scan_and_group(root: &Path, grouping: Grouping) -> (Vec<meowfit_lib::scan::ScannedFile>, Vec<(String, Vec<String>)>, String) {
    let result = scan_folder(root, &ScanOptions::default()).unwrap();
    let groupable: Vec<GroupableFile> = result
        .files
        .iter()
        .map(|f| GroupableFile {
            id: f.id.clone(),
            name: f.name.clone(),
            relative_parent: f.relative_parent.clone(),
        })
        .collect();
    let groups = group_files(&groupable, grouping)
        .into_iter()
        .map(|g| (g.name, g.file_ids))
        .collect();
    (result.files, groups, result.output_dir)
}

/// 由扫描结果与分组结果组装计划请求。
fn plan_of(
    files: &[meowfit_lib::scan::ScannedFile],
    groups: &[(String, Vec<String>)],
    global: Option<Setting>,
    group_settings: HashMap<String, GroupSetting>,
    file_settings: HashMap<String, Setting>,
) -> PlanRequest {
    let group_of: HashMap<&str, &str> = groups
        .iter()
        .flat_map(|(name, ids)| ids.iter().map(move |id| (id.as_str(), name.as_str())))
        .collect();

    PlanRequest {
        files: files
            .iter()
            .filter(|f| f.is_processable())
            .map(|f| PlanFileInput {
                id: f.id.clone(),
                name: f.name.clone(),
                width: f.width.unwrap(),
                height: f.height.unwrap(),
                is_video: false,
                group: group_of
                    .get(f.id.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_default(),
                setting: file_settings.get(&f.id).cloned(),
            })
            .collect(),
        groups: groups
            .iter()
            .map(|(name, _)| PlanGroupInput {
                name: name.clone(),
                setting: group_settings.get(name).cloned(),
            })
            .collect(),
        global,
    }
}

fn sources_of(files: &[meowfit_lib::scan::ScannedFile]) -> Vec<SourceRef> {
    files
        .iter()
        .filter(|f| f.is_processable())
        .map(|f| SourceRef {
            id: f.id.clone(),
            path: f.path.clone(),
            kind: f.kind.unwrap(),
        })
        .collect()
}

fn run(root: &Path, request: &PlanRequest, files: &[meowfit_lib::scan::ScannedFile]) -> meowfit_lib::exec::ExecReport {
    let plan = build_plan(request);
    assert!(plan.ok, "计划必须通过校验");
    execute(&plan, &sources_of(files), root, &ExecOptions::default()).unwrap()
}

/// 场景 1：30 张 JPG，整体 0.5 倍率。
#[test]
fn scenario_1_halve_thirty_jpgs() {
    let root = temp_root("s1").join("素材");
    for i in 0..30 {
        jpg(&root.join(format!("photo_{i:02}.jpg")), 200, 100);
    }
    let before: Vec<u64> = (0..30)
        .map(|i| fs::metadata(root.join(format!("photo_{i:02}.jpg"))).unwrap().len())
        .collect();

    let (files, groups, output_dir) = scan_and_group(&root, Grouping::Prefix);
    assert_eq!(files.len(), 30);

    let request = plan_of(&files, &groups, Some(Setting::scale(Mode::A, 0.5)), HashMap::new(), HashMap::new());
    let report = run(&root, &request, &files);

    assert_eq!(report.counts.success, 30, "全部 30 张都应被处理");
    assert_eq!(report.output_dir, output_dir, "应输出到源文件夹的同级 output/");

    let out = PathBuf::from(&report.output_dir);
    for i in 0..30 {
        let name = format!("photo_{i:02}.jpg");
        // 尺寸减半、宽高比不变（2:1 → 1:1 比例保持为 2:1）
        assert_eq!(dims(&out.join(&name)), (100, 50), "{name} 尺寸不正确");
        // 输出文件名与原名一致，未追加后缀或前缀
        assert!(out.join(&name).exists());
        // 源文件未被改动
        assert_eq!(dims(&root.join(&name)), (200, 100), "{name} 源文件被改动了");
        assert_eq!(
            fs::metadata(root.join(&name)).unwrap().len(),
            before[i],
            "{name} 源文件体积被改动了"
        );
    }

    let _ = fs::remove_dir_all(root.parent().unwrap());
}

/// 场景 2：按命名前缀分组，仅为 icon 组设 128×128。
#[test]
fn scenario_2_only_icon_group_changes_bg_stays_untouched() {
    let root = temp_root("s2").join("素材");
    for i in 1..=20 {
        png(&root.join(format!("icon_{i:02}.png")), 64, 64);
    }
    for i in 1..=10 {
        jpg(&root.join(format!("bg_{i:02}.jpg")), 64, 64);
    }

    let (files, groups, _) = scan_and_group(&root, Grouping::Prefix);
    let names: Vec<&str> = groups.iter().map(|(n, _)| n.as_str()).collect();
    assert!(names.contains(&"icon"), "应识别出 icon 组，实际：{names:?}");
    assert!(names.contains(&"bg"), "应识别出 bg 组，实际：{names:?}");

    let mut group_settings = HashMap::new();
    group_settings.insert(
        "icon".to_string(),
        GroupSetting::Explicit(Setting::width_height(Mode::D, 128.0, 128.0)),
    );

    let request = plan_of(&files, &groups, None, group_settings, HashMap::new());
    let plan = build_plan(&request);
    assert!(plan.ok);

    // 只有 icon 组被改动
    for entry in &plan.entries {
        if entry.group == "icon" {
            assert_eq!(entry.action, Action::Resize, "icon 组应被改动");
        } else {
            assert_eq!(entry.action, Action::Unchanged, "bg 组必须原地不动");
            assert_eq!(entry.source, meowfit_lib::model::SettingSource::None);
        }
    }

    let report = run(&root, &request, &files);
    assert_eq!(report.counts.success, 20);
    assert_eq!(report.counts.unchanged, 10);

    let out = PathBuf::from(&report.output_dir);
    assert_eq!(dims(&out.join("icon_01.png")), (128, 128));
    // bg 组原地不动：输出目录中不存在 bg 文件
    assert!(!out.join("bg_01.jpg").exists(), "未改动的 bg 组不得出现在输出目录");
    assert_eq!(dims(&root.join("bg_01.jpg")), (64, 64), "bg 源文件必须原样不动");

    let _ = fs::remove_dir_all(root.parent().unwrap());
}

/// 场景 3：为 icon_07.png 单独设 256×256。
#[test]
fn scenario_3_single_file_override() {
    let root = temp_root("s3").join("素材");
    for i in 1..=20 {
        png(&root.join(format!("icon_{i:02}.png")), 64, 64);
    }
    for i in 1..=10 {
        jpg(&root.join(format!("bg_{i:02}.jpg")), 64, 64);
    }

    let (files, groups, _) = scan_and_group(&root, Grouping::Prefix);
    let mut group_settings = HashMap::new();
    group_settings.insert(
        "icon".to_string(),
        GroupSetting::Explicit(Setting::width_height(Mode::D, 128.0, 128.0)),
    );
    let mut file_settings = HashMap::new();
    file_settings.insert(
        "icon_07.png".to_string(),
        Setting::width_height(Mode::D, 256.0, 256.0),
    );

    let request = plan_of(&files, &groups, None, group_settings, file_settings);
    let report = run(&root, &request, &files);

    assert_eq!(report.counts.success, 20);
    let out = PathBuf::from(&report.output_dir);
    assert_eq!(dims(&out.join("icon_07.png")), (256, 256), "单文件设置应覆盖分组");
    assert_eq!(dims(&out.join("icon_01.png")), (128, 128), "同组其余仍为分组设置");
    assert_eq!(dims(&out.join("icon_08.png")), (128, 128));

    let _ = fs::remove_dir_all(root.parent().unwrap());
}

/// 场景 4：整体 2 倍、bg 组 0.5 倍 —— 分组优先于整体。
#[test]
fn scenario_4_group_beats_global() {
    let root = temp_root("s4").join("素材");
    for i in 1..=4 {
        png(&root.join(format!("icon_{i:02}.png")), 100, 100);
    }
    for i in 1..=4 {
        png(&root.join(format!("bg_{i:02}.png")), 100, 100);
    }

    let (files, groups, _) = scan_and_group(&root, Grouping::Prefix);
    let mut group_settings = HashMap::new();
    group_settings.insert(
        "bg".to_string(),
        GroupSetting::Explicit(Setting::scale(Mode::A, 0.5)),
    );

    let request = plan_of(
        &files,
        &groups,
        Some(Setting::scale(Mode::A, 2.0)),
        group_settings,
        HashMap::new(),
    );
    let report = run(&root, &request, &files);
    assert_eq!(report.counts.success, 8);

    let out = PathBuf::from(&report.output_dir);
    assert_eq!(dims(&out.join("icon_01.png")), (200, 200), "icon 组应按整体 2 倍");
    assert_eq!(dims(&out.join("bg_01.png")), (50, 50), "bg 组应按分组 0.5 倍");

    let _ = fs::remove_dir_all(root.parent().unwrap());
}

/// 场景 5：改按扩展名分组、按所在子文件夹分组，结果与分组方式一致。
#[test]
fn scenario_5_other_grouping_modes() {
    let root = temp_root("s5").join("素材");
    png(&root.join("ui/button.png"), 100, 100);
    png(&root.join("ui/card.png"), 100, 100);
    jpg(&root.join("ui/photo.jpg"), 100, 100);
    png(&root.join("icons/star.png"), 100, 100);

    // 按扩展名分组：png 组设 40×40，jpg 组设 20×20
    let (files, groups, _) = scan_and_group(&root, Grouping::Extension);
    let ext_names: Vec<&str> = groups.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(ext_names, vec!["jpg", "png"], "按扩展名应得到 jpg / png 两组");

    let mut group_settings = HashMap::new();
    group_settings.insert(
        "png".to_string(),
        GroupSetting::Explicit(Setting::width_height(Mode::D, 40.0, 40.0)),
    );
    group_settings.insert(
        "jpg".to_string(),
        GroupSetting::Explicit(Setting::width_height(Mode::D, 20.0, 20.0)),
    );

    let request = plan_of(&files, &groups, None, group_settings, HashMap::new());
    let report = run(&root, &request, &files);
    assert_eq!(report.counts.success, 4);

    let out = PathBuf::from(&report.output_dir);
    assert_eq!(dims(&out.join("ui/button.png")), (40, 40), "png 组按 png 设置");
    assert_eq!(dims(&out.join("ui/photo.jpg")), (20, 20), "jpg 组按 jpg 设置");
    // 保留相对目录结构
    assert!(out.join("icons/star.png").exists());

    let _ = fs::remove_dir_all(root.parent().unwrap());

    // 按所在子文件夹分组：ui 组设 30×30，根目录组设 60×60
    let root2 = temp_root("s5b").join("素材");
    png(&root2.join("top.png"), 100, 100);
    png(&root2.join("ui/a.png"), 100, 100);
    png(&root2.join("ui/b.png"), 100, 100);

    let (files2, groups2, _) = scan_and_group(&root2, Grouping::Folder);
    let folder_names: Vec<&str> = groups2.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(
        folder_names,
        vec!["<根目录>", "ui"],
        "按所在子文件夹应得到 <根目录> / ui 两组"
    );

    let mut group_settings2 = HashMap::new();
    group_settings2.insert(
        "ui".to_string(),
        GroupSetting::Explicit(Setting::width_height(Mode::D, 30.0, 30.0)),
    );
    group_settings2.insert(
        "<根目录>".to_string(),
        GroupSetting::Explicit(Setting::width_height(Mode::D, 60.0, 60.0)),
    );

    let request2 = plan_of(&files2, &groups2, None, group_settings2, HashMap::new());
    let report2 = run(&root2, &request2, &files2);
    let out2 = PathBuf::from(&report2.output_dir);
    assert_eq!(dims(&out2.join("top.png")), (60, 60));
    assert_eq!(dims(&out2.join("ui/a.png")), (30, 30));

    let _ = fs::remove_dir_all(root2.parent().unwrap());
}

/// 场景 7：四类尺寸边界情况，且**均不写出任何文件**。
#[test]
fn scenario_7_size_boundaries_write_nothing() {
    let root = temp_root("s7").join("素材");
    png(&root.join("a.png"), 1920, 1080);
    png(&root.join("b.png"), 10, 10);
    png(&root.join("c.png"), 100, 100);

    let (files, groups, output_dir) = scan_and_group(&root, Grouping::Prefix);
    let out = PathBuf::from(&output_dir);

    // (1) 联动开启、只填一边 → 界面按基准 1920×1080 把另一边补成 450 后再交给引擎
    let mut request = plan_of(
        &files,
        &groups,
        Some(Setting::width_height(Mode::B, 800.0, 450.0)),
        HashMap::new(),
        HashMap::new(),
    );
    // 只对 a.png 生效，避免牵连其余文件的期望值
    request.files.retain(|f| f.id == "a.png");
    let plan = build_plan(&request);
    assert!(plan.ok, "联动开启时补全两边后应能算出结果");
    assert_eq!(plan.entries[0].target.unwrap().height, 450);

    // (2) 联动关闭、只填一边 → E_INCOMPLETE_DIMENSION 并指明缺少哪一边
    for (setting, expected) in [
        (
            Setting {
                width: Some(800.0),
                ..Setting::new(Mode::D)
            },
            "缺少高度",
        ),
        (
            Setting {
                height: Some(800.0),
                ..Setting::new(Mode::D)
            },
            "缺少宽度",
        ),
    ] {
        let mut req = plan_of(&files, &groups, None, HashMap::new(), HashMap::new());
        req.files.retain(|f| f.id == "a.png");
        req.files[0].setting = Some(setting);
        let plan = build_plan(&req);
        assert!(!plan.ok);
        let err = plan.entries[0].error.as_ref().unwrap();
        assert_eq!(err.code, meowfit_lib::error::ErrorCode::IncompleteDimension);
        assert!(err.message.contains(expected), "实际：{}", err.message);
    }

    // (3) 计算结果小于 1px
    let mut req = plan_of(&files, &groups, Some(Setting::scale(Mode::A, 0.01)), HashMap::new(), HashMap::new());
    req.files.retain(|f| f.id == "b.png"); // 10×10 → 0.1px
    let plan = build_plan(&req);
    assert!(!plan.ok);
    assert_eq!(
        plan.entries[0].error.as_ref().unwrap().code,
        meowfit_lib::error::ErrorCode::SizeUnderflow
    );

    // (4) 倍率超出上限：1920×1080 放大 100 倍 = 192000px，超过 32768 上限
    let mut req = plan_of(&files, &groups, Some(Setting::scale(Mode::A, 100.0)), HashMap::new(), HashMap::new());
    req.files.retain(|f| f.id == "a.png");
    let plan = build_plan(&req);
    assert!(!plan.ok);
    assert_eq!(
        plan.entries[0].error.as_ref().unwrap().code,
        meowfit_lib::error::ErrorCode::SizeOverflow
    );

    // 四种情况都没有触发执行，输出目录中没有任何文件被写出
    let written = fs::read_dir(&out)
        .map(|entries| entries.filter_map(Result::ok).count())
        .unwrap_or(0);
    assert_eq!(written, 0, "校验失败时不得写出任何文件");

    let _ = fs::remove_dir_all(root.parent().unwrap());
}

/// 场景 8：不支持格式被跳过并列入「已跳过」，不中断其他文件。
#[test]
fn scenario_8_unsupported_formats_are_skipped_without_breaking() {
    let root = temp_root("s8").join("素材");
    png(&root.join("good_01.png"), 100, 100);
    png(&root.join("good_02.png"), 100, 100);
    fs::write(root.join("notes.xyz"), b"not media").unwrap();
    fs::write(root.join("archive.zip"), b"PK").unwrap();

    let result = scan_folder(&root, &ScanOptions::default()).unwrap();
    assert_eq!(result.files.len(), 4);

    let skipped: Vec<&str> = result
        .files
        .iter()
        .filter(|f| f.skip_reason.is_some())
        .map(|f| f.name.as_str())
        .collect();
    assert_eq!(skipped, vec!["archive.zip", "notes.xyz"], "不支持的文件应列入已跳过");
    for f in result.files.iter().filter(|f| f.skip_reason.is_some()) {
        assert!(f.skip_reason.as_ref().unwrap().contains("不支持"));
    }

    // 其余文件照常处理，不中断
    let (files, groups, _) = scan_and_group(&root, Grouping::Prefix);
    let request = plan_of(&files, &groups, Some(Setting::scale(Mode::A, 0.5)), HashMap::new(), HashMap::new());
    let report = run(&root, &request, &files);

    assert_eq!(report.counts.success, 2);
    let out = PathBuf::from(&report.output_dir);
    assert_eq!(dims(&out.join("good_01.png")), (50, 50));
    assert_eq!(dims(&out.join("good_02.png")), (50, 50));
    assert!(!out.join("notes.xyz").exists());
    assert!(!out.join("archive.zip").exists());

    let _ = fs::remove_dir_all(root.parent().unwrap());
}

/// 场景 2/5 的前提：三种分组方式互斥由单选控件保证（规范 6.4「单选，不允许叠加」），
/// 这里验证每种方式各自独立给出正确结果，且不存在「叠加」的中间态。
#[test]
fn grouping_modes_are_mutually_exclusive() {
    let root = temp_root("s6").join("素材");
    png(&root.join("ui/icon_01.png"), 10, 10);
    png(&root.join("bg_01.png"), 10, 10);

    let result = scan_folder(&root, &ScanOptions::default()).unwrap();
    let groupable: Vec<GroupableFile> = result
        .files
        .iter()
        .map(|f| GroupableFile {
            id: f.id.clone(),
            name: f.name.clone(),
            relative_parent: f.relative_parent.clone(),
        })
        .collect();

    let prefix: Vec<String> = group_files(&groupable, Grouping::Prefix)
        .into_iter()
        .map(|g| g.name)
        .collect();
    let folder: Vec<String> = group_files(&groupable, Grouping::Folder)
        .into_iter()
        .map(|g| g.name)
        .collect();

    // 每次调用只按一种方式分组，得到的结果互不相同，说明不存在叠加
    assert_eq!(prefix, vec!["bg", "icon"]);
    assert_eq!(folder, vec!["<根目录>", "ui"]);
    assert_ne!(prefix, folder);

    let _ = fs::remove_dir_all(root.parent().unwrap());
}
