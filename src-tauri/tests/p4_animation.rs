//! 规范 16.2 的验收场景 17–19，在真实动图文件上跑完整链路。

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use meowfit_lib::algo::grouping::{group_files, GroupableFile};
use meowfit_lib::algo::plan::build_plan;
use meowfit_lib::exec::{execute, AnimationOptions, ExecOptions, SourceRef};
use meowfit_lib::ffmpeg::{self, FfmpegPaths};
use meowfit_lib::model::{Grouping, Mode, PlanFileInput, PlanRequest, Setting};
use meowfit_lib::scan::{scan_folder, ScannedFile, ScanOptions};

fn paths() -> FfmpegPaths {
    ffmpeg::resolve_paths().expect("需要 FFmpeg 二进制")
}

fn temp_root(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "meowfit-p4-{tag}-{}-{:?}",
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

/// 造一个 500×500、6 帧、每帧 100ms、循环 3 次、右半透明的 GIF。
///
/// 注意必须走调色板链：直接 `-c:v gif` 编 rgba 会把 alpha 丢掉，
/// 造出来的「透明 GIF」其实是全不透明的，测试就失去意义了。
fn build_animated_gif(path: &Path) {
    ff(&[
        "-hide_banner",
        "-nostdin",
        "-y",
        "-f",
        "lavfi",
        "-i",
        "testsrc=s=500x500:r=10:d=0.6,format=rgba,geq=r='r(X,Y)':g='g(X,Y)':b='b(X,Y)':a='if(lt(X,250),255,0)'",
        "-filter_complex",
        "[0:v]split[a][b];[a]palettegen=reserve_transparent=1[p];[b][p]paletteuse",
        "-loop",
        "3",
        &path.to_string_lossy(),
    ]);
}

/// 逐帧延迟序列（GIF 的每帧延迟）。
fn frame_delays(path: &Path) -> Vec<String> {
    let out = Command::new(&paths().ffprobe)
        .args([
            "-v",
            "error",
            "-select_streams",
            "v",
            "-show_entries",
            "frame=duration_time",
            "-of",
            "csv=p=0",
            &path.to_string_lossy(),
        ])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

fn frame_count(path: &Path) -> usize {
    let out = Command::new(&paths().ffprobe)
        .args([
            "-v",
            "error",
            "-select_streams",
            "v",
            "-count_frames",
            "-show_entries",
            "stream=nb_read_frames",
            "-of",
            "csv=p=0",
            &path.to_string_lossy(),
        ])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).trim().parse().unwrap_or(0)
}

fn dims(path: &Path) -> (u32, u32) {
    image::image_dimensions(path).unwrap()
}

fn run_with(
    root: &Path,
    files: &[ScannedFile],
    global: Option<Setting>,
    animation: AnimationOptions,
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
            loop_count: f.animation.as_ref().map(|a| a.loop_count),
            ext: Some(f.ext.clone()),
        skip_reason: None,
        })
        .collect();

    execute(
        &plan,
        &sources,
        root,
        &ExecOptions {
            animation,
            ..ExecOptions::default()
        },
    )
    .unwrap()
}

/// 场景 17：500×500 的 GIF → 250×250，帧数、每帧延迟、循环次数、透明通道均不变。
#[test]
fn scenario_17_gif_resize_preserves_animation() {
    let root = material_root("s17");
    let source = root.join("anim.gif");
    build_animated_gif(&source);

    let before_frames = frame_count(&source);
    let before_delays = frame_delays(&source);
    assert_eq!(before_frames, 6, "测试素材应为 6 帧");
    assert_eq!(before_delays.len(), 6);

    let files = scan_folder(&root, &ScanOptions::default()).unwrap().files;
    // 扫描必须把它认成动图，而不是静态图片
    let scanned = files.iter().find(|f| f.name == "anim.gif").unwrap();
    assert_eq!(
        scanned.kind,
        Some(meowfit_lib::model::MediaKind::Animated),
        "GIF 应被识别为动图"
    );
    assert_eq!(
        scanned.animation.as_ref().map(|a| a.loop_count),
        Some(3),
        "应读出循环次数 3"
    );

    let report = run_with(
        &root,
        &files,
        Some(Setting::scale(Mode::A, 0.5)),
        AnimationOptions::default(),
    );
    assert_eq!(report.counts.success, 1, "{:?}", report.outcomes[0].reason);

    let out = PathBuf::from(&report.output_dir).join("anim.gif");
    // 尺寸减半
    assert_eq!(dims(&out), (250, 250));
    // 帧数不变
    assert_eq!(frame_count(&out), before_frames, "帧数必须保留");
    // 每帧延迟不变
    assert_eq!(frame_delays(&out), before_delays, "每帧延迟必须保留");
    // 循环次数不变
    let after = scan_folder(Path::new(&report.output_dir), &ScanOptions::default()).unwrap().files;
    let out_file = after.iter().find(|f| f.name == "anim.gif").unwrap();
    assert_eq!(
        out_file.animation.as_ref().map(|a| a.loop_count),
        Some(3),
        "循环次数必须保留"
    );

    // 透明通道保留：抽第一帧出来看有透明像素
    let frame_png = root.parent().unwrap().join("frame.png");
    ff(&[
        "-hide_banner",
        "-nostdin",
        "-y",
        "-i",
        &out.to_string_lossy(),
        "-frames:v",
        "1",
        &frame_png.to_string_lossy(),
    ]);
    let img = image::open(&frame_png).unwrap().to_rgba8();
    let has_transparent = img.pixels().any(|p| p[3] < 128);
    let has_opaque = img.pixels().any(|p| p[3] >= 128);
    assert!(has_transparent, "透明区域应保留");
    assert!(has_opaque, "不透明区域应保留");

    // 动画能正常解码
    let decode = Command::new(&paths().ffmpeg)
        .args([
            "-hide_banner",
            "-nostdin",
            "-v",
            "error",
            "-i",
            &out.to_string_lossy(),
            "-f",
            "null",
            "-",
        ])
        .output()
        .unwrap();
    assert!(decode.status.success(), "输出应能完整解码");

    let _ = fs::remove_dir_all(root.parent().unwrap());
}

/// 场景 18：减色至 64 色后体积明显下降；转 MP4 后体积进一步下降。
#[test]
fn scenario_18_fewer_colors_and_video_conversion_shrink_the_file() {
    let root = material_root("s18");
    let source = root.join("anim.gif");
    build_animated_gif(&source);
    let source_size = fs::metadata(&source).unwrap().len();

    let files = scan_folder(&root, &ScanOptions::default()).unwrap().files;
    let run = |animation: AnimationOptions| {
        run_with(&root, &files, Some(Setting::scale(Mode::A, 0.5)), animation)
    };

    // 256 色
    let full = run(AnimationOptions {
        colors: 256,
        ..AnimationOptions::default()
    });
    let full_size = fs::metadata(PathBuf::from(&full.output_dir).join("anim.gif")).unwrap().len();

    // 64 色
    let root2 = material_root("s18b");
    fs::copy(&source, root2.join("anim.gif")).unwrap();
    let files2 = scan_folder(&root2, &ScanOptions::default()).unwrap().files;
    let fewer = run_with(
        &root2,
        &files2,
        Some(Setting::scale(Mode::A, 0.5)),
        AnimationOptions {
            colors: 64,
            ..AnimationOptions::default()
        },
    );
    let fewer_size = fs::metadata(PathBuf::from(&fewer.output_dir).join("anim.gif")).unwrap().len();
    assert!(
        fewer_size < full_size,
        "减色至 64 后体积应下降：{fewer_size} vs {full_size}"
    );

    // 转 MP4
    let root3 = material_root("s18c");
    fs::copy(&source, root3.join("anim.gif")).unwrap();
    let files3 = scan_folder(&root3, &ScanOptions::default()).unwrap().files;
    let video = run_with(
        &root3,
        &files3,
        Some(Setting::scale(Mode::A, 0.5)),
        AnimationOptions {
            colors: 64,
            to_video: "mp4".into(),
            ..AnimationOptions::default()
        },
    );
    assert_eq!(video.counts.success, 1, "{:?}", video.outcomes[0].reason);
    // 输出扩展名随转换改变
    let mp4 = PathBuf::from(&video.output_dir).join("anim.mp4");
    assert!(mp4.exists(), "应输出 anim.mp4");
    let mp4_size = fs::metadata(&mp4).unwrap().len();
    assert!(mp4_size < fewer_size, "转 MP4 后体积应进一步下降：{mp4_size} vs {fewer_size}");

    // 体积对比数据可用（规范 6.10 要求给出体积对比提示）
    let outcome = &video.outcomes[0];
    assert_eq!(outcome.original_size, source_size);
    assert_eq!(outcome.new_size, Some(mp4_size));

    let _ = fs::remove_dir_all(root.parent().unwrap());
    let _ = fs::remove_dir_all(root2.parent().unwrap());
    let _ = fs::remove_dir_all(root3.parent().unwrap());
}

/// 场景 19：设一个可达成目标体积，阶梯依次尝试并在某一档达标。
#[test]
fn scenario_19_size_ladder_reaches_the_target() {
    let root = material_root("s19");
    let source = root.join("anim.gif");
    build_animated_gif(&source);
    let source_size = fs::metadata(&source).unwrap().len();

    let files = scan_folder(&root, &ScanOptions::default()).unwrap().files;

    // 目标取「减半」，逐级降尺寸一定能达到
    let target = source_size / 2;
    let report = run_with(
        &root,
        &files,
        // 用会改变尺寸的倍率：纯粹只设目标体积而不改尺寸会被判为「未改动」
        Some(Setting::scale(Mode::A, 0.9)),
        AnimationOptions {
            target_bytes: Some(target),
            ..AnimationOptions::default()
        },
    );
    assert_eq!(report.counts.success, 1, "{:?}", report.outcomes[0].reason);

    let out = PathBuf::from(&report.output_dir).join("anim.gif");
    let size = fs::metadata(&out).unwrap().len();
    assert!(size <= target, "应达标：{size} ≤ {target}");

    // 每一档的尝试结果都要记录并展示
    let ladder: Vec<&String> = report.notes.iter().filter(|n| n.contains("体积阶梯")).collect();
    assert!(!ladder.is_empty(), "应记录每一档的尝试结果，实际 notes={:?}", report.notes);
    assert!(
        report.notes.iter().any(|n| n.contains("达标")),
        "应记录在哪一档达标，实际 notes={:?}",
        report.notes
    );

    // 不达循环上限：档位数有界
    assert!(ladder.len() <= 12, "档位不应失控，实际 {} 档", ladder.len());

    let _ = fs::remove_dir_all(root.parent().unwrap());
}

/// 场景 19（续）：目标不可能达成时给出「无法达标」并保留最小体积输出，不进入死循环。
#[test]
fn scenario_19_unreachable_target_keeps_smallest_and_reports() {
    let root = material_root("s19b");
    let source = root.join("anim.gif");
    build_animated_gif(&source);

    let files = scan_folder(&root, &ScanOptions::default()).unwrap().files;

    // 目标 1 字节：任何编码都不可能达到
    let report = run_with(
        &root,
        &files,
        Some(Setting::scale(Mode::A, 0.9)),
        AnimationOptions {
            target_bytes: Some(1),
            ..AnimationOptions::default()
        },
    );

    assert_eq!(report.counts.success, 1, "未达标也应保留最小体积输出");
    let out = PathBuf::from(&report.output_dir).join("anim.gif");
    assert!(out.exists(), "输出文件应存在");
    assert!(fs::metadata(&out).unwrap().len() > 1);
    assert!(
        report.notes.iter().any(|n| n.contains("无法达标")),
        "应明确标注未达标，实际 notes={:?}",
        report.notes
    );

    let _ = fs::remove_dir_all(root.parent().unwrap());
}

/// 动态 WebP 必须按动图处理，不能当成静态图片只写出第一帧。
#[test]
fn animated_webp_is_not_treated_as_a_static_image() {
    let root = material_root("webp");
    let source = root.join("anim.webp");
    ff(&[
        "-hide_banner",
        "-nostdin",
        "-y",
        "-f",
        "lavfi",
        "-i",
        "testsrc=s=200x120:r=8:d=0.5",
        "-c:v",
        "libwebp_anim",
        "-loop",
        "0",
        &source.to_string_lossy(),
    ]);

    let files = scan_folder(&root, &ScanOptions::default()).unwrap().files;
    let scanned = files.iter().find(|f| f.name == "anim.webp").unwrap();
    assert_eq!(
        scanned.kind,
        Some(meowfit_lib::model::MediaKind::Animated),
        "动态 WebP 必须按容器内容判为动图"
    );

    // 静态 WebP 仍应判为静态
    let still = root.join("still.webp");
    ff(&[
        "-hide_banner",
        "-nostdin",
        "-y",
        "-i",
        &source.to_string_lossy(),
        "-frames:v",
        "1",
        "-c:v",
        "libwebp",
        &still.to_string_lossy(),
    ]);
    let files = scan_folder(&root, &ScanOptions::default()).unwrap().files;
    let still_file = files.iter().find(|f| f.name == "still.webp").unwrap();
    assert_eq!(
        still_file.kind,
        Some(meowfit_lib::model::MediaKind::Raster),
        "静态 WebP 应仍按静态图片处理"
    );

    let _ = fs::remove_dir_all(root.parent().unwrap());
}
