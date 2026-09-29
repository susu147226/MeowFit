//! P3 中可以在**没有 FFmpeg 的机器上**验证的部分。
//!
//! 真实视频编码需要 ffmpeg.exe / ffprobe.exe，本机与仓库当前都没有，
//! 因此验收场景 13–16 尚未实测（见提交说明）。这里覆盖的是：
//! 探测结果的解析、命令构造（在 src/ffmpeg.rs 的单测里）、以及缺少 FFmpeg 时
//! 程序必须**如实告知**而不是静默失败或崩溃。

use std::fs;
use std::path::PathBuf;

use meowfit_lib::commands;
use meowfit_lib::ffmpeg;
use meowfit_lib::scan::{scan_folder, ScanOptions};

fn temp_root(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "meowfit-p3-{tag}-{}-{:?}",
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

/// 缺 FFmpeg 时，视频必须被列入「已跳过」并说明原因，而不是让整个扫描崩掉。
#[test]
fn videos_are_skipped_with_a_clear_reason_when_ffmpeg_is_absent() {
    let root = temp_root("novideo");
    fs::write(root.join("clip.mp4"), vec![0u8; 128]).unwrap();
    fs::write(root.join("clip.mkv"), vec![0u8; 128]).unwrap();
    image::RgbImage::from_pixel(8, 8, image::Rgb([1, 2, 3]))
        .save(root.join("ok.png"))
        .unwrap();

    let result = scan_folder(&root, &ScanOptions::default()).unwrap();
    assert_eq!(result.files.len(), 3);

    // 图片照常可用
    let image_file = result.files.iter().find(|f| f.name == "ok.png").unwrap();
    assert!(image_file.is_processable());

    for name in ["clip.mp4", "clip.mkv"] {
        let file = result.files.iter().find(|f| f.name == name).unwrap();
        let reason = file.skip_reason.clone().unwrap_or_default();
        if ffmpeg::resolve_paths().is_ok() {
            // 本机有 FFmpeg：假文件会因无法解析而被跳过，同样要给出原因
            assert!(!reason.is_empty(), "{name} 应有跳过原因");
        } else {
            assert!(
                reason.contains("FFmpeg"),
                "{name} 的跳过原因应说明缺少 FFmpeg，实际：{reason}"
            );
        }
        assert!(file.width.is_none());
    }

    let _ = fs::remove_dir_all(&root);
}

/// FFmpeg 自检命令在任何环境下都必须返回结果（而不是抛错让界面无从提示）。
#[test]
fn self_check_command_always_returns_a_report() {
    let report = commands::ffmpeg_self_check();
    if !report.ffmpeg_found {
        // 未找到时必须给出可操作的说明与完整缺失清单
        assert!(report.summary.contains("FFmpeg"), "实际：{}", report.summary);
        assert_eq!(report.missing_encoders.len(), ffmpeg::REQUIRED_ENCODERS.len());
        assert_eq!(report.missing_filters.len(), ffmpeg::REQUIRED_FILTERS.len());
        assert!(report.missing_hevc_decoder);
        assert!(!report.all_present());
    } else {
        // 找到了就应当给出真实的自检结论
        assert!(!report.summary.is_empty());
    }
}

/// 缺少 FFmpeg 时执行视频任务必须报 `E_FFMPEG_MISSING`，而不是写出半成品。
#[test]
fn executing_a_video_without_ffmpeg_fails_loudly() {
    if ffmpeg::resolve_paths().is_ok() {
        // 本机装了 FFmpeg 时这条不适用（会在别的场景里覆盖）
        return;
    }

    let root = temp_root("execvideo");
    fs::write(root.join("clip.mp4"), vec![0u8; 256]).unwrap();

    // 直接构造计划：跳过探测，模拟一个已知尺寸的视频
    let request: meowfit_lib::model::PlanRequest = serde_json::from_value(serde_json::json!({
        "files": [{
            "id": "clip.mp4",
            "name": "clip.mp4",
            "width": 1920,
            "height": 1080,
            "isVideo": true,
            "group": "clip",
            "setting": null
        }],
        "groups": [],
        "global": { "mode": "A", "scale": 0.5 }
    }))
    .unwrap();

    let plan = meowfit_lib::algo::plan::build_plan(&request);
    assert!(plan.ok, "计划本身应当通过校验（尺寸计算不依赖 FFmpeg）");

    let sources = vec![meowfit_lib::exec::SourceRef {
        id: "clip.mp4".into(),
        path: root.join("clip.mp4").to_string_lossy().into_owned(),
        kind: meowfit_lib::model::MediaKind::Video,
    }];

    let report = meowfit_lib::exec::execute(
        &plan,
        &sources,
        &root,
        &meowfit_lib::exec::ExecOptions::default(),
    )
    .expect("执行本身不应整体失败，失败应逐文件记录");

    // 该文件计入失败，且原因是缺少 FFmpeg
    assert_eq!(report.counts.failed, 1);
    assert_eq!(report.counts.success, 0);
    let reason = report.outcomes[0].reason.clone().unwrap_or_default();
    assert!(reason.contains("E_FFMPEG_MISSING"), "实际：{reason}");

    // 输出目录里不得留下任何文件
    let out = PathBuf::from(&report.output_dir);
    let written = fs::read_dir(&out)
        .map(|e| e.filter_map(Result::ok).count())
        .unwrap_or(0);
    assert_eq!(written, 0, "失败时不得留下半成品");

    let _ = fs::remove_dir_all(root.parent().unwrap());
}

/// 视频的尺寸计算走的是同一套纯函数：强制偶数、按观感方向计算。
#[test]
fn video_dimensions_use_the_same_size_algorithm() {
    // 1921×1081 视频缩小到 1 倍仍然要向下取偶
    let setting = meowfit_lib::model::Setting::scale(meowfit_lib::model::Mode::A, 1.0);
    let computed = meowfit_lib::algo::size::compute_target(1921, 1081, &setting, true)
        .unwrap()
        .unwrap();
    assert_eq!((computed.width, computed.height), (1920, 1080));

    // 缩放到 720p 时宽高都必须是偶数
    let target = meowfit_lib::model::Setting::width_height(meowfit_lib::model::Mode::D, 1281.0, 721.0);
    let computed = meowfit_lib::algo::size::compute_target(1920, 1080, &target, true)
        .unwrap()
        .unwrap();
    assert_eq!((computed.width, computed.height), (1280, 720));
    assert_eq!(computed.width % 2, 0);
    assert_eq!(computed.height % 2, 0);
}
