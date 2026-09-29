//! 前后端接口契约测试。
//!
//! 界面通过 `invoke` 发送的 JSON 必须与 Rust 侧签名和 serde 命名严格一致。
//! 这里用与 `src/store.ts`、`src/types.ts` 完全相同的 JSON 字面量驱动命令函数，
//! 并校验返回值的字段名，从而在没有图形界面的情况下覆盖 IPC 边界。

use std::fs;
use std::path::{Path, PathBuf};

use image::{Rgb, RgbImage};
use serde_json::json;

use meowfit_lib::commands;
use meowfit_lib::model::{Grouping, Setting};

fn temp_root(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "meowfit-ipc-{tag}-{}-{:?}",
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
    RgbImage::from_pixel(w, h, Rgb([120, 160, 200])).save(path).unwrap();
}

/// 界面发送的 ScanOptions 形状必须能被命令反序列化。
#[test]
fn scan_options_shape_matches_frontend() {
    let options: meowfit_lib::scan::ScanOptions = serde_json::from_value(json!({
        "recursive": true,
        "include": ["icon_*"],
        "exclude": ["*_thumb.*", "*@2x*"]
    }))
    .expect("ScanOptions 应接受界面发送的 camelCase 形状");

    assert!(options.recursive);
    assert_eq!(options.include, vec!["icon_*"]);
    assert_eq!(options.exclude.len(), 2);
}

/// 界面发送的 PlanRequest 形状（含三态与 Setting 的 camelCase 字段）必须能被反序列化。
#[test]
fn plan_request_shape_matches_frontend() {
    let value = json!({
        "files": [
            {
                "id": "icon_01.png",
                "name": "icon_01.png",
                "width": 1920,
                "height": 1080,
                "isVideo": false,
                "group": "icon",
                "setting": {
                    "mode": "B",
                    "width": 800,
                    "height": 450,
                    "noPad": true,
                    "onlyDown": false,
                    "onlyUp": false
                }
            }
        ],
        "groups": [
            { "name": "icon", "setting": { "kind": "followGlobal" } },
            { "name": "bg", "setting": null },
            {
                "name": "logo",
                "setting": { "kind": "explicit", "setting": { "mode": "A", "scale": 0.5 } }
            }
        ],
        "global": { "mode": "A", "scale": 2 }
    });

    let request: meowfit_lib::model::PlanRequest =
        serde_json::from_value(value).expect("PlanRequest 应接受界面发送的形状");

    assert_eq!(request.files.len(), 1);
    assert_eq!(request.files[0].group, "icon");
    assert_eq!(request.files[0].setting.as_ref().unwrap().no_pad, true);
    assert_eq!(request.groups.len(), 3);
    assert_eq!(request.global.as_ref().unwrap().scale, Some(2.0));

    // 单文件 B 800×450 + noPad → 内容尺寸 800×450；而 1920×1080 的 fit 比例是 800/1920
    let c = meowfit_lib::algo::size::compute_target(1920, 1080, request.files[0].setting.as_ref().unwrap(), false)
        .unwrap()
        .unwrap();
    assert_eq!((c.width, c.height), (800, 450));
}

/// 三种三态写法都必须被接受：null / followGlobal / explicit。
#[test]
fn group_tier_three_states_round_trip() {
    use meowfit_lib::model::GroupSetting;

    let unset: Option<GroupSetting> = serde_json::from_value(json!(null)).unwrap();
    assert!(unset.is_none());

    let follow: Option<GroupSetting> =
        serde_json::from_value(json!({ "kind": "followGlobal" })).unwrap();
    assert!(matches!(follow, Some(GroupSetting::FollowGlobal)));

    let explicit: Option<GroupSetting> = serde_json::from_value(json!({
        "kind": "explicit",
        "setting": { "mode": "D", "width": 128, "height": 128 }
    }))
    .unwrap();
    match explicit {
        Some(GroupSetting::Explicit(s)) => {
            assert_eq!(s.width, Some(128.0));
            assert_eq!(s.height, Some(128.0));
        }
        other => panic!("应解析为 explicit，实际 {other:?}"),
    }
}

/// `Setting` 序列化回界面的字段名必须是 camelCase，界面才能正确回显。
#[test]
fn setting_serializes_to_camel_case() {
    let setting = Setting {
        no_pad: true,
        only_down: true,
        only_up: true,
        ..Setting::width_height(meowfit_lib::model::Mode::B, 800.0, 600.0)
    };
    let value = serde_json::to_value(&setting).unwrap();
    assert_eq!(value["noPad"], json!(true));
    assert_eq!(value["onlyDown"], json!(true));
    assert_eq!(value["onlyUp"], json!(true));
    assert_eq!(value["mode"], json!("B"));
    assert_eq!(value["width"], json!(800.0));
}

/// `Grouping` 取值为界面使用的字符串。
#[test]
fn grouping_values_match_frontend() {
    assert_eq!(serde_json::to_value(Grouping::Prefix).unwrap(), json!("prefix"));
    assert_eq!(serde_json::to_value(Grouping::Extension).unwrap(), json!("extension"));
    assert_eq!(serde_json::to_value(Grouping::Folder).unwrap(), json!("folder"));
    assert_eq!(
        serde_json::from_value::<Grouping>(json!("folder")).unwrap(),
        Grouping::Folder
    );
}

/// 错误码必须是 13.2 表格里的 `E_*` 写法。
#[test]
fn error_codes_use_spec_strings() {
    use meowfit_lib::error::ErrorCode;
    for (code, expected) in [
        (ErrorCode::UnsupportedFormat, "E_UNSUPPORTED_FORMAT"),
        (ErrorCode::SizeUnderflow, "E_SIZE_UNDERFLOW"),
        (ErrorCode::SizeOverflow, "E_SIZE_OVERFLOW"),
        (ErrorCode::IncompleteDimension, "E_INCOMPLETE_DIMENSION"),
        (ErrorCode::OutputUnwritable, "E_OUTPUT_UNWRITABLE"),
        (ErrorCode::TargetUnreachable, "E_TARGET_UNREACHABLE"),
    ] {
        assert_eq!(serde_json::to_value(code).unwrap(), json!(expected));
        assert_eq!(code.as_str(), expected);
    }
}

/// 走一遍命令层：扫描 → 分组 → 预览 → 执行，返回值的字段名与界面类型一致。
#[test]
fn command_layer_end_to_end() {
    let root = temp_root("cmd").join("素材");
    png(&root.join("icon_01.png"), 100, 100);
    png(&root.join("icon_02.png"), 100, 100);
    png(&root.join("bg_01.png"), 100, 100);

    let root_str = root.to_string_lossy().into_owned();

    // scan_folder
    let scan = commands::scan_folder(root_str.clone(), None).expect("扫描应成功");
    assert_eq!(scan.files.len(), 3);
    let scan_value = serde_json::to_value(&scan).unwrap();
    assert!(scan_value["outputDir"].is_string(), "ScanResult 应带 outputDir");
    assert_eq!(scan_value["files"][0]["relativePath"].is_string(), true);
    assert!(scan_value["files"][0].get("skipReason").is_some());

    // group_entries
    let groupable: Vec<meowfit_lib::algo::grouping::GroupableFile> = scan
        .files
        .iter()
        .map(|f| meowfit_lib::algo::grouping::GroupableFile {
            id: f.id.clone(),
            name: f.name.clone(),
            relative_parent: f.relative_parent.clone(),
        })
        .collect();
    let groups = commands::group_entries(groupable, Grouping::Prefix);
    let names: Vec<&str> = groups.iter().map(|g| g.name.as_str()).collect();
    assert_eq!(names, vec!["bg", "icon"]);
    let groups_value = serde_json::to_value(&groups).unwrap();
    assert!(groups_value[0]["fileIds"].is_array(), "Group 应使用 fileIds");

    // preview_plan：仅为 icon 组设 128×128
    let request: meowfit_lib::model::PlanRequest = serde_json::from_value(json!({
        "files": scan.files.iter().map(|f| json!({
            "id": f.id,
            "name": f.name,
            "width": f.width.unwrap(),
            "height": f.height.unwrap(),
            "isVideo": false,
            "group": if f.name.starts_with("icon") { "icon" } else { "bg" },
            "setting": null
        })).collect::<Vec<_>>(),
        "groups": [
            { "name": "icon", "setting": { "kind": "explicit", "setting": { "mode": "D", "width": 128, "height": 128 } } },
            { "name": "bg", "setting": null }
        ],
        "global": null
    }))
    .unwrap();

    let plan = commands::preview_plan(request.clone());
    assert!(plan.ok);
    let plan_value = serde_json::to_value(&plan).unwrap();
    assert_eq!(plan_value["ok"], json!(true));

    // 文件按相对路径排序，bg_01.png 在前；逐条按其分组核对行为
    let icon = plan.entries.iter().find(|e| e.name == "icon_01.png").unwrap();
    assert_eq!(icon.original_width, 100);
    assert_eq!(icon.action, meowfit_lib::model::Action::Resize);
    assert_eq!(icon.source, meowfit_lib::model::SettingSource::Group);
    let bg = plan.entries.iter().find(|e| e.name == "bg_01.png").unwrap();
    assert_eq!(bg.action, meowfit_lib::model::Action::Unchanged);
    assert_eq!(bg.source, meowfit_lib::model::SettingSource::None);

    // 序列化后的字段名与界面类型一致
    let entry_value = serde_json::to_value(icon).unwrap();
    assert_eq!(entry_value["originalWidth"], json!(100));
    assert_eq!(entry_value["action"], json!("resize"));
    assert_eq!(entry_value["source"], json!("group"));

    // execute_plan
    let sources: Vec<meowfit_lib::exec::SourceRef> = scan
        .files
        .iter()
        .filter(|f| f.is_processable())
        .map(|f| meowfit_lib::exec::SourceRef {
            id: f.id.clone(),
            path: f.path.clone(),
            kind: f.kind.unwrap(),
        })
        .collect();

    let report = commands::execute_plan(request, sources, None, root_str).expect("执行应成功");
    assert_eq!(report.counts.success, 2);
    assert_eq!(report.counts.unchanged, 1);

    let report_value = serde_json::to_value(&report).unwrap();
    assert!(report_value["outputDir"].is_string());
    assert_eq!(report_value["counts"]["unchanged"], json!(1));
    assert_eq!(report_value["outcomes"][0]["status"].is_string(), true);

    // bg 组原地不动，不复制到输出目录
    let out = PathBuf::from(&report.output_dir);
    assert!(out.join("icon_01.png").exists());
    assert!(!out.join("bg_01.png").exists());

    let _ = fs::remove_dir_all(root.parent().unwrap());
}

/// 校验失败时命令层必须拒绝执行，且不写出任何文件。
#[test]
fn command_layer_refuses_invalid_plan() {
    let root = temp_root("cmd-bad").join("素材");
    png(&root.join("a.png"), 10, 10);
    let root_str = root.to_string_lossy().into_owned();

    let request: meowfit_lib::model::PlanRequest = serde_json::from_value(json!({
        "files": [{
            "id": "a.png",
            "name": "a.png",
            "width": 10,
            "height": 10,
            "isVideo": false,
            "group": "g",
            "setting": null
        }],
        "groups": [],
        "global": { "mode": "A", "scale": 0.01 }
    }))
    .unwrap();

    assert!(!commands::preview_plan(request.clone()).ok);

    let sources = vec![meowfit_lib::exec::SourceRef {
        id: "a.png".into(),
        path: root.join("a.png").to_string_lossy().into_owned(),
        kind: meowfit_lib::model::MediaKind::Raster,
    }];
    let err = commands::execute_plan(request, sources, None, root_str).unwrap_err();
    assert!(err.contains("校验失败"), "实际错误：{err}");

    let out = root.parent().unwrap().join("output");
    let written = fs::read_dir(&out)
        .map(|e| e.filter_map(Result::ok).count())
        .unwrap_or(0);
    assert_eq!(written, 0, "校验失败时不得写出任何文件");

    let _ = fs::remove_dir_all(root.parent().unwrap());
}

/// ExecOptions 的界面形状必须被接受（含 P2 的图片处理参数）。
#[test]
fn exec_options_shape_matches_frontend() {
    let options: meowfit_lib::exec::ExecOptions = serde_json::from_value(json!({
        "outputDir": null,
        "keepStructure": true,
        "onConflict": "skip",
        "image": {
            "resample": "nearest",
            "quality": 92,
            "format": "webp",
            "keepAllMetadata": true,
            "backgroundFill": "#F0EAD8"
        }
    }))
    .expect("ExecOptions 应接受界面发送的形状");

    assert!(options.output_dir.is_none());
    assert!(options.keep_structure);
    assert_eq!(options.on_conflict, "skip");
    assert_eq!(options.image.resample, meowfit_lib::imaging::Resample::Nearest);
    assert_eq!(options.image.quality, 92);
    assert_eq!(options.image.format, meowfit_lib::imaging::OutputFormat::Webp);
    assert!(options.image.keep_all_metadata);
    assert_eq!(options.image.background_fill, "#F0EAD8");
}

/// Settings 的界面形状必须与 Rust 侧一致（含 P2 新增的两个字段）。
#[test]
fn settings_shape_matches_frontend() {
    let settings: meowfit_lib::config::Settings = serde_json::from_value(json!({
        "version": 1,
        "language": "zh-CN",
        "theme": { "mode": "dark", "backgroundImage": null, "backgroundOpacity": 80 },
        "output": {
            "directory": "./output",
            "overwriteSource": false,
            "keepStructure": true,
            "onConflict": "rename",
            "backgroundFillColor": "#FFFFFF",
            "stripRedundantMetadata": true,
            "outputFormat": "webp"
        },
        "processing": {
            "concurrency": 4,
            "resample": "bilinear",
            "jpgQuality": 80,
            "videoCrf": 23,
            "videoEncoder": "h264",
            "hardwareAccel": false,
            "hdrTonemapToSdr": false,
            "gifColors": 256,
            "gifDither": false,
            "upscaleWarnThreshold": 4,
            "svgDpi": 192,
            "svgSizeMode": "dpi"
        },
        "grouping": "folder",
        "recentFolders": ["D:/a"]
    }))
    .expect("Settings 应接受界面发送的形状");

    assert_eq!(settings.output.output_format, "webp");
    assert_eq!(settings.processing.svg_size_mode, "dpi");
    assert_eq!(settings.processing.svg_dpi, 192);
    assert_eq!(settings.processing.resample, "bilinear");

    // 回写界面时字段名保持 camelCase
    let value = serde_json::to_value(&settings).unwrap();
    assert_eq!(value["output"]["outputFormat"], json!("webp"));
    assert_eq!(value["processing"]["svgSizeMode"], json!("dpi"));
    assert_eq!(value["recentFolders"][0], json!("D:/a"));
}

/// 外观设置的界面形状与默认值（规范 6.14 / 第七节）。
#[test]
fn appearance_settings_shape_and_defaults() {
    // 界面发送的形状
    let settings: meowfit_lib::config::Settings = serde_json::from_value(json!({
        "version": 1,
        "language": "zh-CN",
        "theme": {
            "mode": "system",
            "backgroundImage": "D:/预览/bg.png",
            "backgroundOpacity": 40,
            "autoScrim": false,
            "accent": "#2F6F4F",
            "density": "compact",
            "sidebarWidth": 300,
            "previewWidth": 420
        },
        "output": {
            "directory": "./output",
            "overwriteSource": false,
            "keepStructure": true,
            "onConflict": "skip",
            "backgroundFillColor": "#FFFFFF",
            "stripRedundantMetadata": true,
            "outputFormat": "keep"
        },
        "processing": {
            "concurrency": 8,
            "resample": "lanczos3",
            "jpgQuality": 85,
            "videoCrf": 23,
            "videoEncoder": "h264",
            "hardwareAccel": false,
            "hdrTonemapToSdr": false,
            "gifColors": 256,
            "gifDither": false,
            "upscaleWarnThreshold": 4,
            "svgDpi": 96,
            "svgSizeMode": "pixel"
        },
        "grouping": "prefix",
        "recentFolders": []
    }))
    .expect("Settings 应接受界面发送的外观形状");

    assert_eq!(settings.theme.mode, "system");
    assert_eq!(settings.theme.background_opacity, 40);
    assert!(!settings.theme.auto_scrim);
    assert_eq!(settings.theme.density, "compact");
    assert_eq!(settings.theme.preview_width, 420);

    // 回写界面时字段名保持 camelCase
    let value = serde_json::to_value(&settings).unwrap();
    assert_eq!(value["theme"]["backgroundImage"], json!("D:/预览/bg.png"));
    assert_eq!(value["theme"]["sidebarWidth"], json!(300));

    // 默认外观：跟随系统、无背景图、标准密度、天蓝色主题、默认布局
    let fresh = meowfit_lib::config::Settings::default();
    assert_eq!(fresh.theme.mode, "system", "外观默认必须跟随系统");
    assert!(fresh.theme.background_image.is_none());
    assert_eq!(fresh.theme.density, "standard");
    assert!(fresh.theme.auto_scrim, "默认应开启自动加蒙层");
    assert_eq!(fresh.theme.sidebar_width, 350);
    assert_eq!(fresh.theme.preview_width, 350);
    assert_eq!(fresh.theme.run_height, 280);
    assert_eq!(fresh.theme.log_height, 190);
    assert_eq!(
        fresh.theme.accent, "#2F8BD0",
        "默认主题色应为天蓝色，且该值在两套外观下都不需要校正"
    );

    // 旧配置缺字段时退回默认值而不是加载失败
    let legacy: meowfit_lib::config::Settings = serde_json::from_value(json!({
        "version": 1,
        "language": "zh-CN",
        "grouping": "prefix"
    }))
    .expect("缺字段的旧配置应能加载");
    assert_eq!(
        legacy.theme.mode, "system",
        "旧配置没有 theme 段时，外观应落到默认的「跟随系统」"
    );
    assert_eq!(legacy.theme.density, "standard");
}

/// 背景图读取：不支持的扩展名必须被拒绝，且不会读取文件内容。
#[test]
fn background_image_rejects_unsupported_extension() {
    let dir = std::env::temp_dir().join(format!("meowfit-bg-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("bg.txt");
    std::fs::write(&path, b"not an image").unwrap();

    let err = meowfit_lib::commands::read_background_image(path.to_string_lossy().into_owned())
        .unwrap_err();
    assert!(err.contains("仅支持"), "实际错误：{err}");

    // 不存在的文件报读取失败而不是 panic
    let missing = dir.join("nope.png");
    assert!(
        meowfit_lib::commands::read_background_image(missing.to_string_lossy().into_owned()).is_err()
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// 背景图读取成功后应返回可直接渲染的 data URL。
#[test]
fn background_image_returns_data_url() {
    let dir = std::env::temp_dir().join(format!("meowfit-bgok-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("bg.png");
    image::RgbImage::from_pixel(4, 4, image::Rgb([10, 20, 30]))
        .save(&path)
        .unwrap();

    let url = meowfit_lib::commands::read_background_image(path.to_string_lossy().into_owned())
        .expect("支持的图片格式应能读取");
    assert!(url.starts_with("data:image/png;base64,"), "实际：{}", &url[..40]);

    let _ = std::fs::remove_dir_all(&dir);
}
#[test]
fn legacy_settings_without_p2_fields_still_loads() {
    let settings: meowfit_lib::config::Settings = serde_json::from_value(json!({
        "version": 1,
        "language": "zh-CN",
        "grouping": "prefix",
        "output": {
            "directory": "./output",
            "overwriteSource": false,
            "keepStructure": true,
            "onConflict": "skip",
            "backgroundFillColor": "#FFFFFF",
            "stripRedundantMetadata": true
        },
        "processing": {
            "concurrency": 8,
            "resample": "lanczos3",
            "jpgQuality": 85,
            "videoCrf": 23,
            "videoEncoder": "h264",
            "hardwareAccel": false,
            "hdrTonemapToSdr": false,
            "gifColors": 256,
            "gifDither": false,
            "upscaleWarnThreshold": 4,
            "svgDpi": 96
        }
    }))
    .expect("缺少 P2 字段的旧配置应能加载");

    assert_eq!(settings.output.output_format, "keep");
    assert_eq!(settings.processing.svg_size_mode, "pixel");
}
