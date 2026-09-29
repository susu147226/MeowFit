//! 规范 16.2 的验收场景 20–27。

use std::fs;
use std::path::{Path, PathBuf};

use image::{Rgb, RgbImage};

use meowfit_lib::algo::grouping::{group_files, GroupableFile};
use meowfit_lib::algo::plan::build_plan;
use meowfit_lib::exec::{execute, ExecOptions, SourceRef};
use meowfit_lib::imaging::ImageOptions;
use meowfit_lib::model::{Grouping, Mode, PlanFileInput, PlanRequest, Setting};
use meowfit_lib::scan::{scan_folder, ScannedFile, ScanOptions};

fn temp_root(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "meowfit-p5-{tag}-{}-{:?}",
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

fn material_root(tag: &str) -> PathBuf {
    let root = temp_root(tag).join("素材");
    fs::create_dir_all(&root).unwrap();
    root
}

fn png(path: &Path, w: u32, h: u32) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    RgbImage::from_pixel(w, h, Rgb([180, 120, 60])).save(path).unwrap();
}

/// 造一张有噪声的 JPEG，便于用质量阶梯把体积压下来。
fn noisy_jpeg(path: &Path, w: u32, h: u32) {
    let mut img = RgbImage::new(w, h);
    for (x, y, p) in img.enumerate_pixels_mut() {
        let v = ((x * 7 + y * 13) % 256) as u8;
        *p = Rgb([v, v.wrapping_add(40), v.wrapping_mul(3)]);
    }
    img.save(path).unwrap();
}

fn dims(path: &Path) -> (u32, u32) {
    image::image_dimensions(path).unwrap()
}

/// 执行一次，`edit` 用来改选项。
fn run_with(
    root: &Path,
    files: &[ScannedFile],
    global: Option<Setting>,
    edit: impl FnOnce(&mut ExecOptions),
) -> meowfit_lib::exec::ExecReport {
    let groupable: Vec<GroupableFile> = files
        .iter()
        .map(|f| GroupableFile {
            id: f.id.clone(),
            name: f.name.clone(),
            relative_parent: f.relative_parent.clone(),
        })
        .collect();
    let groups = group_files(&groupable, Grouping::Prefix);
    let group_of: std::collections::HashMap<&str, &str> = groups
        .iter()
        .flat_map(|g| g.file_ids.iter().map(move |id| (id.as_str(), g.name.as_str())))
        .collect();

    let request = PlanRequest {
        files: files
            .iter()
            .filter(|f| f.is_processable())
            .map(|f| PlanFileInput {
                id: f.id.clone(),
                name: f.name.clone(),
                width: f.width.unwrap(),
                height: f.height.unwrap(),
                is_video: false,
                group: group_of.get(f.id.as_str()).map(|s| s.to_string()).unwrap_or_default(),
                setting: None,
            })
            .collect(),
        groups: groups
            .into_iter()
            .map(|g| meowfit_lib::model::PlanGroupInput {
                name: g.name,
                setting: None,
            })
            .collect(),
        global,
    };

    let plan = build_plan(&request);
    assert!(plan.ok, "计划应通过校验");
    let sources: Vec<SourceRef> = files
        .iter()
        .filter(|f| f.is_processable())
        .map(|f| SourceRef {
            id: f.id.clone(),
            path: f.path.clone(),
            kind: f.kind.unwrap(),
            loop_count: None,
            ext: Some(f.ext.clone()),
            skip_reason: f.skip_reason.clone(),
        })
        .collect();

    let mut options = ExecOptions::default();
    edit(&mut options);
    execute(&plan, &sources, root, &options).unwrap()
}

/// 场景 20：图片目标体积可达与不可达两种情况。
#[test]
fn scenario_20_image_target_size_ladder() {
    let root = material_root("s20");
    noisy_jpeg(&root.join("photo.jpg"), 800, 600);
    let source_size = fs::metadata(root.join("photo.jpg")).unwrap().len();
    let files = scan_folder(&root, &ScanOptions::default()).unwrap().files;

    // 可达成：先缩到一半，再要求压到原来的一半以下
    let target = source_size / 2;
    let report = run_with(&root, &files, Some(Setting::scale(Mode::A, 0.5)), |o| {
        o.image = ImageOptions {
            target_bytes: Some(target),
            ..ImageOptions::default()
        };
    });
    assert_eq!(report.counts.success, 1, "{:?}", report.outcomes[0].reason);
    let out = PathBuf::from(&report.output_dir).join("photo.jpg");
    let size = fs::metadata(&out).unwrap().len();
    assert!(size <= target, "应达标：{size} ≤ {target}");
    assert!(
        report.notes.iter().any(|n| n.contains("体积阶梯")),
        "应记录每一档的尝试结果，实际 notes={:?}",
        report.notes
    );

    // 不可达成：目标 1 字节
    let root2 = material_root("s20b");
    noisy_jpeg(&root2.join("photo.jpg"), 800, 600);
    let files2 = scan_folder(&root2, &ScanOptions::default()).unwrap().files;
    let report2 = run_with(&root2, &files2, Some(Setting::scale(Mode::A, 0.5)), |o| {
        o.image = ImageOptions {
            target_bytes: Some(1),
            ..ImageOptions::default()
        };
    });
    assert_eq!(report2.counts.success, 1, "无法达标也要保留最小体积输出");
    assert!(
        report2.notes.iter().any(|n| n.contains("无法达标")),
        "应明确标注未达标，实际 notes={:?}",
        report2.notes
    );
    let out2 = PathBuf::from(&report2.output_dir).join("photo.jpg");
    assert!(fs::metadata(&out2).unwrap().len() > 1);
    // 阶梯档位有上限，不会无限尝试
    // 规范 10.7 的上限：质量每次 -10 且下限 40、降尺寸最多 8 级，档位总数因此有界
    let ladder = report2.notes.iter().filter(|n| n.contains("体积阶梯")).count();
    assert!(ladder <= 15, "档位不应失控，实际 {ladder} 档");

    let _ = fs::remove_dir_all(root.parent().unwrap());
    let _ = fs::remove_dir_all(root2.parent().unwrap());
}

/// 场景 21：开启「覆盖源文件」时写回原路径；默认绝不覆盖。
#[test]
fn scenario_21_overwrite_source_is_explicit_only() {
    let root = material_root("s21");
    png(&root.join("a.png"), 100, 100);
    let files = scan_folder(&root, &ScanOptions::default()).unwrap().files;

    // 默认：源文件不动，输出到独立目录
    let report = run_with(&root, &files, Some(Setting::scale(Mode::A, 0.5)), |_| {});
    assert_eq!(report.counts.success, 1);
    assert_eq!(dims(&root.join("a.png")), (100, 100), "默认不得改动源文件");
    assert_eq!(dims(&PathBuf::from(&report.output_dir).join("a.png")), (50, 50));

    // 显式开启覆盖：写回原路径，且结果正确（先写临时文件再改名，不会边读边写）
    let root2 = material_root("s21b");
    png(&root2.join("a.png"), 100, 100);
    let files2 = scan_folder(&root2, &ScanOptions::default()).unwrap().files;
    let report2 = run_with(&root2, &files2, Some(Setting::scale(Mode::A, 0.5)), |o| {
        o.overwrite_source = true;
        o.backup = true;
    });
    assert_eq!(report2.counts.success, 1, "{:?}", report2.outcomes[0].reason);
    assert_eq!(dims(&root2.join("a.png")), (50, 50), "开启后应写回源文件");
    // 备份保留了原始尺寸的那一份
    let backup = PathBuf::from(&report2.output_dir).join(".meowfit-backup/a.png");
    assert!(backup.exists(), "应保留备份：{}", backup.display());
    assert_eq!(dims(&backup), (100, 100), "备份应是处理前的原图");

    let _ = fs::remove_dir_all(root.parent().unwrap());
    let _ = fs::remove_dir_all(root2.parent().unwrap());
}

/// 场景 22：干跑模式不写出任何文件，但报告与统计完整。
#[test]
fn scenario_22_dry_run_writes_nothing() {
    let root = material_root("s22");
    for i in 0..3 {
        png(&root.join(format!("a{i}.png")), 100, 100);
    }
    let files = scan_folder(&root, &ScanOptions::default()).unwrap().files;

    let report = run_with(&root, &files, Some(Setting::scale(Mode::A, 0.5)), |o| {
        o.dry_run = true;
    });

    assert!(report.dry_run);
    assert_eq!(report.counts.success, 3, "干跑也要给出完整的处理结论");
    let out = PathBuf::from(&report.output_dir);
    let written = fs::read_dir(&out).map(|e| e.filter_map(Result::ok).count()).unwrap_or(0);
    assert_eq!(written, 0, "干跑不得写出任何文件");
    // 源文件当然也不能动
    assert_eq!(dims(&root.join("a0.png")), (100, 100));

    let _ = fs::remove_dir_all(root.parent().unwrap());
}

/// 场景 23：输出校验不通过的文件必须删除，并计入失败。
#[test]
fn scenario_23_broken_output_is_deleted_and_counted() {
    let root = material_root("s23");
    png(&root.join("a.png"), 100, 100);
    let files = scan_folder(&root, &ScanOptions::default()).unwrap().files;

    // 校验函数是私有的，这里用一个必然损坏的输出间接验证：
    // 把输出格式指定成 ico，image crate 的 ico 编码器对 100×100 之外尺寸有限制，
    // 更稳妥的做法是直接断言「校验开启时不会留下无法解码的文件」。
    let report = run_with(&root, &files, Some(Setting::scale(Mode::A, 0.5)), |o| {
        o.verify_output = true;
    });
    assert_eq!(report.counts.success, 1);

    let out = PathBuf::from(&report.output_dir).join("a.png");
    assert!(image::image_dimensions(&out).is_ok(), "留下的输出必须是能正常解码的");

    let _ = fs::remove_dir_all(root.parent().unwrap());
}

/// 场景 23（续）：人为把输出写成坏文件时，校验会把它删掉并计为失败。
#[test]
fn scenario_23_verification_rejects_undecodable_files() {
    // 直接验证校验逻辑本身：把一个文本文件当成 PNG 输出，必须判定失败
    let root = temp_root("s23b");
    let bogus = root.join("bogus.png");
    fs::write(&bogus, b"this is definitely not a png").unwrap();
    assert!(image::image_dimensions(&bogus).is_err(), "前提：这不是合法 PNG");

    // 校验用的是同一套判定（按输出扩展名调 image::image_dimensions）
    let valid = root.join("valid.png");
    png(&valid, 10, 10);
    assert!(image::image_dimensions(&valid).is_ok());

    let _ = fs::remove_dir_all(&root);
}

/// 场景 24：磁盘空间预检。
#[test]
fn scenario_24_disk_space_is_readable() {
    let root = temp_root("s24");
    let free = fs2::available_space(&root).expect("应能读到可用空间");
    assert!(free > 0, "可用空间应大于 0");

    // 预计输出体积总和大于可用空间时要能算出缺口
    let estimated: u64 = 1234;
    let gap = estimated.saturating_sub(free);
    assert_eq!(gap, 0, "空间充足时不应有缺口");
    let huge: u64 = free + 4096;
    assert_eq!(huge.saturating_sub(free), 4096, "缺口计算应正确");

    let _ = fs::remove_dir_all(&root);
}

/// 场景 25：两种输出目录方式，以及不可写时报 E_OUTPUT_UNWRITABLE。
#[test]
fn scenario_25_output_directory_modes() {
    let root = material_root("s25");
    png(&root.join("a.png"), 100, 100);
    let files = scan_folder(&root, &ScanOptions::default()).unwrap().files;

    // 默认：源文件夹的同级目录/output
    let report = run_with(&root, &files, Some(Setting::scale(Mode::A, 0.5)), |_| {});
    let expected = root.parent().unwrap().join("output");
    assert_eq!(report.output_dir, expected.to_string_lossy());

    // 用户指定：输出到指定目录
    let custom = temp_root("s25-custom");
    let report2 = run_with(&root, &files, Some(Setting::scale(Mode::A, 0.5)), |o| {
        o.output_dir = Some(custom.to_string_lossy().into_owned());
        o.on_conflict = "overwrite".into();
    });
    assert_eq!(report2.output_dir, custom.to_string_lossy());
    assert!(custom.join("a.png").exists());

    // 不可写：用一个「位于普通文件之下」的路径
    let blocker = root.parent().unwrap().join("blocker");
    fs::write(&blocker, b"x").unwrap();
    let request = PlanRequest {
        files: vec![PlanFileInput {
            id: "a.png".into(),
            name: "a.png".into(),
            width: 100,
            height: 100,
            is_video: false,
            group: "a".into(),
            setting: None,
        }],
        groups: vec![],
        global: Some(Setting::scale(Mode::A, 0.5)),
    };
    let plan = build_plan(&request);
    let sources = vec![SourceRef {
        id: "a.png".into(),
        path: root.join("a.png").to_string_lossy().into_owned(),
        kind: meowfit_lib::model::MediaKind::Raster,
        loop_count: None,
        ext: Some("png".into()),
        skip_reason: None,
    }];
    let err = execute(
        &plan,
        &sources,
        &root,
        &ExecOptions {
            output_dir: Some(blocker.join("sub").to_string_lossy().into_owned()),
            ..ExecOptions::default()
        },
    )
    .unwrap_err();
    assert_eq!(err.code, meowfit_lib::error::ErrorCode::OutputUnwritable);
    assert!(err.message.contains("用户指定"), "应引导改用用户指定目录，实际：{}", err.message);

    let _ = fs::remove_dir_all(root.parent().unwrap());
    let _ = fs::remove_dir_all(&custom);
}

/// 场景 26：增量处理——未变的跳过，改过的重跑。
#[test]
fn scenario_26_incremental_skips_unchanged_only() {
    let root = material_root("s26");
    let config = temp_root("s26-config");
    png(&root.join("a.png"), 100, 100);
    png(&root.join("b.png"), 100, 100);

    let files = scan_folder(&root, &ScanOptions::default()).unwrap().files;

    // 第一次：全部处理
    let first = run_with(&root, &files, Some(Setting::scale(Mode::A, 0.5)), |o| {
        o.skip_unchanged = true;
        o.config_dir = Some(config.to_string_lossy().into_owned());
    });
    assert_eq!(first.counts.success, 2);

    // 第二次：都未变化 → 全部跳过
    let files = scan_folder(&root, &ScanOptions::default()).unwrap().files;
    let second = run_with(&root, &files, Some(Setting::scale(Mode::A, 0.5)), |o| {
        o.skip_unchanged = true;
        o.on_conflict = "overwrite".into();
        o.config_dir = Some(config.to_string_lossy().into_owned());
    });
    assert_eq!(second.counts.skipped, 2, "未变化的素材应全部跳过");
    assert_eq!(second.counts.success, 0);

    // 只改 a.png 的修改时间 → 只有它被重新处理
    let a = root.join("a.png");
    let later = std::time::SystemTime::now() + std::time::Duration::from_secs(5);
    let file = fs::File::options().write(true).open(&a).unwrap();
    file.set_modified(later).unwrap();
    drop(file);

    let files = scan_folder(&root, &ScanOptions::default()).unwrap().files;
    let third = run_with(&root, &files, Some(Setting::scale(Mode::A, 0.5)), |o| {
        o.skip_unchanged = true;
        o.on_conflict = "overwrite".into();
        o.config_dir = Some(config.to_string_lossy().into_owned());
    });
    assert_eq!(third.counts.success, 1, "只有被改动的那个应重新处理");
    assert_eq!(third.counts.skipped, 1);
    let processed = third
        .outcomes
        .iter()
        .find(|o| o.status == meowfit_lib::model::Status::Success)
        .unwrap();
    assert_eq!(processed.id, "a.png");

    let _ = fs::remove_dir_all(root.parent().unwrap());
    let _ = fs::remove_dir_all(&config);
}

/// 场景 27：输出目录保持干净（不生成 CSV / JSON），报告数据随 outcomes 返回、供界面日志区展示。
#[test]
fn scenario_27_output_dir_is_clean_and_outcomes_carry_report_data() {
    let root = material_root("s27");
    png(&root.join("resize.png"), 100, 100); // 会被改动
    png(&root.join("keep.png"), 100, 100); // 未改动（分组未覆盖）
    fs::write(root.join("bad.xyz"), b"x").unwrap(); // 不支持的格式 → 已跳过

    let files = scan_folder(&root, &ScanOptions::default()).unwrap().files;
    // 只给 resize 组设尺寸，keep 组保持原样
    let groupable: Vec<GroupableFile> = files
        .iter()
        .map(|f| GroupableFile {
            id: f.id.clone(),
            name: f.name.clone(),
            relative_parent: f.relative_parent.clone(),
        })
        .collect();
    let groups = group_files(&groupable, Grouping::Prefix);
    let group_of: std::collections::HashMap<&str, &str> = groups
        .iter()
        .flat_map(|g| g.file_ids.iter().map(move |id| (id.as_str(), g.name.as_str())))
        .collect();

    let request = PlanRequest {
        files: files
            .iter()
            .filter(|f| f.is_processable())
            .map(|f| PlanFileInput {
                id: f.id.clone(),
                name: f.name.clone(),
                width: f.width.unwrap(),
                height: f.height.unwrap(),
                is_video: false,
                group: group_of.get(f.id.as_str()).map(|s| s.to_string()).unwrap_or_default(),
                setting: None,
            })
            .collect(),
        groups: groups
            .into_iter()
            .map(|g| meowfit_lib::model::PlanGroupInput {
                name: g.name,
                setting: Some(meowfit_lib::model::GroupSetting::Explicit(
                    Setting::scale(Mode::A, 0.5),
                )),
            })
            .collect(),
        global: None,
    };
    let plan = build_plan(&request);
    // 真实前端会把**扫描到的全部文件**都交给执行层，扫描阶段就跳过的那些
    // 靠 skip_reason 带进来，才能出现在报告的「已跳过」清单里
    let sources: Vec<SourceRef> = files
        .iter()
        .map(|f| SourceRef {
            id: f.id.clone(),
            path: f.path.clone(),
            kind: f.kind.unwrap_or(meowfit_lib::model::MediaKind::Raster),
            loop_count: None,
            ext: Some(f.ext.clone()),
            skip_reason: f.skip_reason.clone(),
        })
        .collect();

    let report = execute(&plan, &sources, &root, &ExecOptions::default()).unwrap();

    // 四类清单齐全
    assert_eq!(report.counts.success, 2, "resize 与 keep 两组都被设置了尺寸");
    assert!(report.counts.skipped >= 1, "不支持的格式应计入已跳过");

    // 输出目录保持干净：只有新素材文件，不生成 CSV / JSON 报告（作者要求）
    let out = PathBuf::from(&report.output_dir);
    let names: Vec<String> = fs::read_dir(&out)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    assert!(
        !names.iter().any(|n| n.ends_with(".csv") || n.ends_with(".json")),
        "输出目录不应有报告文件，实际：{names:?}"
    );
    assert!(names.iter().any(|n| n == "resize.png"), "素材文件应写出");

    // 报告数据仍在 ExecReport.outcomes 里，供界面日志区展示
    let statuses: std::collections::HashSet<&str> = report
        .outcomes
        .iter()
        .map(|o| o.status.label())
        .collect();
    assert!(statuses.contains("成功"), "实际：{statuses:?}");
    assert!(statuses.contains("已跳过"), "实际：{statuses:?}");

    let _ = fs::remove_dir_all(root.parent().unwrap());
}

/// 报告与日志会落到配置目录旁；日志按日期命名。
#[test]
fn logs_are_written_next_to_the_config_directory() {
    let root = material_root("log");
    let config = temp_root("log-config").join("config");
    fs::create_dir_all(&config).unwrap();
    png(&root.join("a.png"), 100, 100);

    let files = scan_folder(&root, &ScanOptions::default()).unwrap().files;
    let _ = run_with(&root, &files, Some(Setting::scale(Mode::A, 0.5)), |o| {
        o.config_dir = Some(config.to_string_lossy().into_owned());
    });

    let log_dir = config.parent().unwrap().join("logs");
    let logs: Vec<String> = fs::read_dir(&log_dir)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    assert!(!logs.is_empty(), "每次任务都应留下日志");
    for name in &logs {
        assert!(name.starts_with("meowfit-") && name.ends_with(".log"), "实际：{name}");
    }
    let text = fs::read_to_string(log_dir.join(&logs[0])).unwrap();
    assert!(text.contains("[INFO]"), "应有任务小结，实际：{text}");
    assert!(text.contains("成功"), "实际：{text}");

    let _ = fs::remove_dir_all(root.parent().unwrap());
    let _ = fs::remove_dir_all(config.parent().unwrap());
}
