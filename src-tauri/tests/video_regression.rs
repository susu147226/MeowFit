//! 视频回归测试：data 流（tmcd）导致的转码失败，以及「仅转编码」功能。

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use meowfit_lib::algo::plan::build_plan;
use meowfit_lib::exec::{execute, ExecOptions, SourceRef, VideoOptions};
use meowfit_lib::ffmpeg::{self, FfmpegPaths};
use meowfit_lib::model::{PlanRequest, Setting, Mode};
use meowfit_lib::scan::{scan_folder, ScanOptions};

fn paths() -> FfmpegPaths {
    ffmpeg::resolve_paths().expect("需要 FFmpeg 二进制")
}

fn temp_root(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "meowfit-video-{tag}-{}-{:?}",
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

/// 素材根目录：让每个测试的输出目录（同级 output/）互相独立。
fn material_root(tag: &str) -> PathBuf {
    let root = temp_root(tag).join("素材");
    fs::create_dir_all(&root).unwrap();
    root
}

fn ff(args: &[&str]) {
    let out = Command::new(&paths().ffmpeg)
        .args(args)
        .output()
        .expect("无法运行 ffmpeg");
    assert!(
        out.status.success(),
        "ffmpeg 失败：{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn codec_of(path: &Path) -> String {
    let probe = ffmpeg::probe(&paths(), path).unwrap();
    probe
        .video_stream()
        .and_then(|s| s.codec_name.clone())
        .unwrap_or_default()
}

fn dims_of(path: &Path) -> (u32, u32) {
    let probe = ffmpeg::probe(&paths(), path).unwrap();
    let info = probe.to_video_info().unwrap();
    (info.width, info.height)
}

/// 造一个带 tmcd（timecode data）流的视频。这类流 codec_type=unknown，
/// 旧版 `-map 0` 会把它一起映射导致整个转换失败。
fn build_tmcd_video(path: &Path) {
    ff(&[
        "-hide_banner",
        "-nostdin",
        "-y",
        "-f",
        "lavfi",
        "-i",
        "testsrc=s=640x360:d=1:r=25",
        "-c:v",
        "libx264",
        "-crf",
        "30",
        "-preset",
        "ultrafast",
        "-timecode",
        "00:00:00:00",
        &path.to_string_lossy(),
    ]);
}

fn sources_of(files: &[meowfit_lib::scan::ScannedFile]) -> Vec<SourceRef> {
    files
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
        .collect()
}

fn run(
    root: &Path,
    files: &[meowfit_lib::scan::ScannedFile],
    global: Option<Setting>,
    video: VideoOptions,
) -> meowfit_lib::exec::ExecReport {
    let request = PlanRequest {
        files: files
            .iter()
            .filter(|f| f.is_processable())
            .map(|f| meowfit_lib::model::PlanFileInput {
                id: f.id.clone(),
                name: f.name.clone(),
                width: f.width.unwrap(),
                height: f.height.unwrap(),
                is_video: f.kind == Some(meowfit_lib::model::MediaKind::Video),
                group: "g".into(),
                setting: None,
            })
            .collect(),
        groups: vec![],
        global,
    };
    let plan = build_plan(&request);
    execute(
        &plan,
        &sources_of(files),
        root,
        &ExecOptions {
            video,
            ..ExecOptions::default()
        },
    )
    .unwrap()
}

/// 带 tmcd data 流的视频，转码不应失败（规范 12.1 的 -map 只映射 v/a/s 流）。
#[test]
fn video_with_tmcd_data_stream_transcodes_successfully() {
    let root = material_root("tmcd");
    let source = root.join("clip.mov");
    build_tmcd_video(&source);

    // 确认确实带 tmcd data 流（ffprobe JSON 里 codec_type 为 data / unknown，非视频/音频/字幕）
    let probe = ffmpeg::probe(&paths(), &source).unwrap();
    let has_data = probe.streams.iter().any(|s| {
        !matches!(
            s.codec_type.as_deref(),
            Some("video") | Some("audio") | Some("subtitle")
        )
    });
    assert!(has_data, "测试素材应带 tmcd data 流，实际流：{:?}", probe.streams.iter().map(|s| s.codec_type.clone()).collect::<Vec<_>>());

    let files = scan_folder(&root, &ScanOptions::default()).unwrap().files;
    assert_eq!(files.len(), 1);
    assert!(files[0].is_processable(), "视频应被正常扫描，实际：{:?}", files[0].skip_reason);

    // 缩放转码：旧版 -map 0 会在这里失败
    let report = run(
        &root,
        &files,
        Some(Setting::scale(Mode::A, 0.5)),
        VideoOptions::default(),
    );
    assert_eq!(report.counts.success, 1, "转码应成功：{:?}", report.outcomes[0].reason);

    let out = PathBuf::from(&report.output_dir).join("clip.mov");
    assert_eq!(dims_of(&out), (320, 180), "尺寸应减半");

    let _ = fs::remove_dir_all(&root);
}

/// 「仅转编码」：视频未设置宽高时，也按所选编码器转码，且保持原尺寸。
#[test]
fn transcode_only_converts_codec_without_resizing() {
    let root = material_root("transcode");
    let source = root.join("clip.mp4");
    // 造一个 H.264 视频
    ff(&[
        "-hide_banner",
        "-nostdin",
        "-y",
        "-f",
        "lavfi",
        "-i",
        "testsrc=s=640x360:d=1:r=25",
        "-c:v",
        "libx264",
        "-crf",
        "30",
        "-preset",
        "ultrafast",
        &source.to_string_lossy(),
    ]);
    assert_eq!(codec_of(&source), "h264");

    let files = scan_folder(&root, &ScanOptions::default()).unwrap().files;

    // 不设尺寸、开启仅转编码，目标编码器 H.265
    let report = run(
        &root,
        &files,
        None,
        VideoOptions {
            codec: "h265".into(),
            transcode_only: true,
            ..VideoOptions::default()
        },
    );

    assert_eq!(
        report.counts.success, 1,
        "仅转编码应处理视频：{:?}",
        report.outcomes[0].reason
    );

    let out = PathBuf::from(&report.output_dir).join("clip.mp4");
    // 尺寸保持原样
    assert_eq!(dims_of(&out), (640, 360), "仅转编码不应改变尺寸");
    // 编码器已换成 H.265
    assert_eq!(codec_of(&out), "hevc", "应转为 H.265");

    let _ = fs::remove_dir_all(&root);
}

/// 未开启「仅转编码」时，视频不设尺寸应原地不动（不转码）。
#[test]
fn without_transcode_only_unset_video_stays_untouched() {
    let root = material_root("noop");
    let source = root.join("clip.mp4");
    ff(&[
        "-hide_banner",
        "-nostdin",
        "-y",
        "-f",
        "lavfi",
        "-i",
        "testsrc=s=320x180:d=1:r=25",
        "-c:v",
        "libx264",
        "-crf",
        "30",
        "-preset",
        "ultrafast",
        &source.to_string_lossy(),
    ]);

    let files = scan_folder(&root, &ScanOptions::default()).unwrap().files;

    // 不设尺寸、不开启仅转编码：视频原地不动
    let report = run(&root, &files, None, VideoOptions::default());
    assert_eq!(report.counts.unchanged, 1, "未设尺寸且未开仅转编码，视频应原地不动");

    let out = PathBuf::from(&report.output_dir);
    assert!(!out.join("clip.mp4").exists(), "不应写出输出文件");

    let _ = fs::remove_dir_all(&root);
}
