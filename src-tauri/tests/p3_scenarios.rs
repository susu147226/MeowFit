//! 规范 16.2 的验收场景 13–16，在真实文件上跑完整链路（需要 FFmpeg 二进制）。
//!
//! 与 p2_images.rs 一样，这里走的是界面调用的同一套函数。

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use image::{Rgb, RgbImage};

use meowfit_lib::algo::grouping::{group_files, GroupableFile};
use meowfit_lib::algo::plan::build_plan;
use meowfit_lib::exec::{execute, ExecOptions, SourceRef, VideoOptions};
use meowfit_lib::ffmpeg::{self, FfmpegPaths, ProbeResult};
use meowfit_lib::model::{Grouping, Mode, PlanFileInput, PlanRequest, Setting};
use meowfit_lib::scan::{scan_folder, ScannedFile, ScanOptions};

fn paths() -> FfmpegPaths {
    ffmpeg::resolve_paths().expect(
        "需要 FFmpeg：请把 ffmpeg.exe / ffprobe.exe / libgcc_s_seh-1.dll 放到 \
         src-tauri/resources/ffmpeg/win-x64/",
    )
}

fn temp_root(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "meowfit-p3s-{tag}-{}-{:?}",
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

/// 用项目的 FFmpeg 跑一条命令。
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

fn probe(path: &Path) -> ProbeResult {
    ffmpeg::probe(&paths(), path).expect("ffprobe 应能解析")
}

fn dims_of(probe: &ProbeResult) -> (u32, u32) {
    let info = probe.to_video_info().expect("应有视频流");
    (info.width, info.height)
}

fn stream_codec(probe: &ProbeResult, kind: &str) -> Option<String> {
    probe
        .streams
        .iter()
        .find(|s| s.codec_type.as_deref() == Some(kind))
        .and_then(|s| s.codec_name.clone())
}

fn field(probe: &ProbeResult, name: &str) -> Option<String> {
    probe
        .video_stream()
        .and_then(|s| match name {
            "color_transfer" => s.color_transfer.clone(),
            "color_space" => s.color_space.clone(),
            _ => None,
        })
}

/// 把音轨原样抽出来，用于判断是否被重新编码（`-c copy` 时字节应完全一致）。
fn extract_audio(src: &Path, out: &Path) {
    if out.exists() {
        let _ = fs::remove_file(out);
    }
    ff(&[
        "-hide_banner",
        "-nostdin",
        "-y",
        "-i",
        &src.to_string_lossy(),
        "-map",
        "0:a:0",
        "-c",
        "copy",
        "-f",
        "adts",
        &out.to_string_lossy(),
    ]);
}

/// 能否正常解码出一帧（判断「可正常播放」）。
fn can_decode(path: &Path) -> bool {
    Command::new(&paths().ffmpeg)
        .args([
            "-hide_banner",
            "-nostdin",
            "-v",
            "error",
            "-i",
            &path.to_string_lossy(),
            "-frames:v",
            "1",
            "-f",
            "null",
            "-",
        ])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// 抽一帧成 PNG，供像素级比较（ffmpeg 默认会按显示矩阵摆正）。
fn extract_frame(src: &Path, out: &Path) {
    if out.exists() {
        let _ = fs::remove_file(out);
    }
    ff(&[
        "-hide_banner",
        "-nostdin",
        "-y",
        "-i",
        &src.to_string_lossy(),
        "-frames:v",
        "1",
        &out.to_string_lossy(),
    ]);
}

fn run_with(
    root: &Path,
    files: &[ScannedFile],
    global: Option<Setting>,
    video: VideoOptions,
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
    let group_of: HashMap<&str, &str> = groups
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
                is_video: f.kind == Some(meowfit_lib::model::MediaKind::Video),
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
                ext: None,
                skip_reason: None,
            })
        .collect();

    execute(
        &plan,
        &sources,
        root,
        &ExecOptions {
            video,
            ..ExecOptions::default()
        },
    )
    .unwrap()
}

/// 造一个 1920×1080、3 秒、带 AAC 音频 + mov_text 字幕 + 2 个章节的 MP4。
fn build_source_mp4(path: &Path) {
    let dir = path.parent().unwrap();
    let srt = dir.join("subs.srt");
    let chapters = dir.join("chapters.txt");
    fs::write(
        &srt,
        "1\n00:00:00,000 --> 00:00:01,500\n你好 MeowFit\n\n2\n00:00:01,500 --> 00:00:03,000\n第二段字幕\n",
    )
    .unwrap();
    fs::write(
        &chapters,
        ";FFMETADATA1\n[CHAPTER]\nTIMEBASE=1/1000\nSTART=0\nEND=1500\ntitle=第一段\n\
         [CHAPTER]\nTIMEBASE=1/1000\nSTART=1500\nEND=3000\ntitle=第二段\n",
    )
    .unwrap();

    ff(&[
        "-hide_banner",
        "-nostdin",
        "-y",
        "-f",
        "lavfi",
        "-i",
        "testsrc=size=1920x1080:rate=25:duration=3",
        "-f",
        "lavfi",
        "-i",
        "sine=frequency=440:duration=3",
        "-i",
        &srt.to_string_lossy(),
        "-i",
        &chapters.to_string_lossy(),
        "-map",
        "0:v",
        "-map",
        "1:a",
        "-map",
        "2:s",
        "-map_chapters",
        "3",
        "-c:v",
        "libx264",
        "-crf",
        "30",
        "-preset",
        "ultrafast",
        "-c:a",
        "aac",
        "-c:s",
        "mov_text",
        &path.to_string_lossy(),
    ]);
}

/// 给 MP4 写入 90° 显示矩阵。
///
/// 新版 FFmpeg 已移除写入 `rotate` 标签的能力（`-display_rotation` 只作输入选项），
/// 因此这里直接改写 `tkhd` 盒里的 3×3 变换矩阵——这正是 FFmpeg 自己读的那个矩阵。
fn tag_rotation_90(path: &Path) {
    let mut data = fs::read(path).unwrap();
    let at = data
        .windows(4)
        .position(|w| w == b"tkhd")
        .expect("MP4 里应能找到 tkhd 盒");
    let box_start = at - 4;
    let version = data[box_start + 8];
    // 矩阵位置 = 盒头 8 + 版本与标志 4 + (v1:32 / v0:20) + 保留与音量等 16
    let offset = box_start + if version == 1 { 60 } else { 48 };

    let matrix: [i32; 9] = [0, 65536, 0, -65536, 0, 0, 0, 0, 1073741824];
    for (index, value) in matrix.iter().enumerate() {
        let start = offset + index * 4;
        data[start..start + 4].copy_from_slice(&value.to_be_bytes());
    }
    fs::write(path, data).unwrap();
}

/// 场景 13：1920×1080 的 MP4 → 1280×720。
#[test]
fn scenario_13_downscale_keeps_audio_subs_chapters_and_playability() {
    let root = material_root("s13");
    let source = root.join("source.mp4");
    build_source_mp4(&source);

    let before = probe(&source);
    assert_eq!(dims_of(&before), (1920, 1080));
    assert_eq!(stream_codec(&before, "audio").as_deref(), Some("aac"));
    assert!(before.has_subtitle(), "测试素材应带字幕流");
    assert_eq!(before.chapters.len(), 2);

    let files = scan_folder(&root, &ScanOptions::default()).unwrap().files;
    let report = run_with(
        &root,
        &files,
        Some(Setting::width_height(Mode::D, 1280.0, 720.0)),
        VideoOptions::default(),
    );
    assert_eq!(report.counts.success, 1, "应处理成功：{:?}", report.outcomes[0].reason);

    let out = PathBuf::from(&report.output_dir).join("source.mp4");
    let after = probe(&out);

    // 分辨率正确
    assert_eq!(dims_of(&after), (1280, 720));
    // 时长一致
    assert!(
        (after.duration_sec() - before.duration_sec()).abs() < 0.2,
        "时长应一致：{} vs {}",
        before.duration_sec(),
        after.duration_sec()
    );
    // 字幕与章节均保留
    assert!(after.has_subtitle(), "字幕流应保留");
    assert_eq!(after.chapters.len(), 2, "章节应保留");
    // 可正常播放
    assert!(can_decode(&out), "输出应能正常解码");

    // 音频为直接复制未重编码：把两条音轨各自原样抽出，字节应完全一致
    let a_before = root.join("a_before.aac");
    let a_after = root.join("a_after.aac");
    extract_audio(&source, &a_before);
    extract_audio(&out, &a_after);
    let bytes_before = fs::read(&a_before).unwrap();
    let bytes_after = fs::read(&a_after).unwrap();
    assert!(!bytes_before.is_empty());
    assert_eq!(
        bytes_before, bytes_after,
        "音频必须是直接复制，重编码后字节不会完全一致"
    );

    let _ = fs::remove_dir_all(root.parent().unwrap());
}

/// 场景 14：带旋转元数据的视频，输出观感方向与原视频一致。
#[test]
fn scenario_14_rotation_metadata_is_honoured() {
    let root = material_root("s14");

    // 存储为横向 640×360，左半红、右半蓝；声明旋转 90° 后正向应为 360×640，
    // 且存储图的左缘会转到正向的顶部。
    let mut stored = RgbImage::new(640, 360);
    for (x, _y, p) in stored.enumerate_pixels_mut() {
        *p = if x < 320 { Rgb([220, 30, 30]) } else { Rgb([30, 60, 200]) };
    }
    let png = root.join("stored.png");
    stored.save(&png).unwrap();

    let rotated = root.join("rotated.mp4");
    ff(&[
        "-hide_banner",
        "-nostdin",
        "-y",
        "-loop",
        "1",
        "-i",
        &png.to_string_lossy(),
        "-t",
        "2",
        "-c:v",
        "libx264",
        "-crf",
        "30",
        "-preset",
        "ultrafast",
        "-pix_fmt",
        "yuv420p",
        &rotated.to_string_lossy(),
    ]);
    // 写入 90° 显示矩阵：存储横向 640×360，正向应为 360×640
    let tagged = root.join("tagged.mp4");
    fs::copy(&rotated, &tagged).unwrap();
    tag_rotation_90(&tagged);
    fs::remove_file(&rotated).unwrap();
    // 造素材用的中间图片不能留在素材夹里，否则会一起被处理
    fs::remove_file(&png).unwrap();

    let before = probe(&tagged);
    assert_eq!(before.rotation(), 90, "测试素材应带 90° 旋转元数据");
    assert!(before.swaps_axes());

    // 扫描必须按观感方向报告宽高
    let files = scan_folder(&root, &ScanOptions::default()).unwrap().files;
    let scanned = files.iter().find(|f| f.name == "tagged.mp4").unwrap();
    assert_eq!(
        (scanned.width, scanned.height),
        (Some(360), Some(640)),
        "应按观感方向报告宽高"
    );

    // 缩到一半：360×640 → 180×320
    let report = run_with(
        &root,
        &files,
        Some(Setting::scale(Mode::A, 0.5)),
        VideoOptions::default(),
    );
    assert_eq!(report.counts.success, 1, "{:?}", report.outcomes[0].reason);

    let out = PathBuf::from(&report.output_dir).join("tagged.mp4");
    let after = probe(&out);
    assert_eq!(dims_of(&after), (180, 320), "输出应为正向 180×320");
    assert_eq!(after.rotation(), 0, "输出不应再带旋转元数据");

    // 观感方向一致：分别抽第一帧（ffmpeg 会按显示矩阵摆正）比较上下颜色
    let scratch = root.parent().unwrap().to_path_buf();
    let ref_png = scratch.join("ref.png");
    let out_png = scratch.join("out.png");
    extract_frame(&tagged, &ref_png);
    extract_frame(&out, &out_png);

    let px = |path: &Path, fy: f64| {
        let img = image::open(path).unwrap().to_rgba8();
        let (w, h) = img.dimensions();
        img.get_pixel(w / 2, (h as f64 * fy) as u32).0
    };
    let ref_top = px(&ref_png, 0.1);
    let ref_bottom = px(&ref_png, 0.9);
    let out_top = px(&out_png, 0.1);
    let out_bottom = px(&out_png, 0.9);

    assert!(ref_top[0] > ref_top[2], "原视频正向顶部应为红：{ref_top:?}");
    assert!(ref_bottom[2] > ref_bottom[0], "原视频正向底部应为蓝：{ref_bottom:?}");
    assert!(
        out_top[0] > out_top[2],
        "输出顶部应与原视频一致（红），实际 {out_top:?}"
    );
    assert!(
        out_bottom[2] > out_bottom[0],
        "输出底部应与原视频一致（蓝），实际 {out_bottom:?}"
    );

    let _ = fs::remove_dir_all(root.parent().unwrap());
}

/// 场景 15：HDR 默认原样传递；选择色调映射后转为 SDR。
#[test]
fn scenario_15_hdr_is_preserved_by_default_and_tonemapped_on_demand() {
    let root = material_root("s15");
    let hdr = root.join("hdr.mp4");
    ff(&[
        "-hide_banner",
        "-nostdin",
        "-y",
        "-f",
        "lavfi",
        "-i",
        "testsrc=size=640x360:rate=25:duration=2",
        "-c:v",
        "libx265",
        "-preset",
        "ultrafast",
        "-pix_fmt",
        "yuv420p10le",
        // x265 需要从 x265-params 传色彩信息，FFmpeg 顶层的 -color_trc 不会写进 VUI
        "-x265-params",
        "colorprim=bt2020:transfer=smpte2084:colormatrix=bt2020nc\
         :master-display=G(13250,34500)B(7500,3000)R(34000,16000)WP(15635,16450)L(10000000,1)\
         :max-cll=1000,400",
        "-tag:v",
        "hvc1",
        &hdr.to_string_lossy(),
    ]);

    let before = probe(&hdr);
    assert!(before.is_hdr(), "测试素材应被判定为 HDR");
    assert_eq!(field(&before, "color_transfer").as_deref(), Some("smpte2084"));

    let files = scan_folder(&root, &ScanOptions::default()).unwrap().files;

    // 默认：保持 HDR 原样传递，色彩元数据不被改写
    let report = run_with(
        &root,
        &files,
        Some(Setting::scale(Mode::A, 0.5)),
        VideoOptions::default(),
    );
    assert_eq!(report.counts.success, 1, "{:?}", report.outcomes[0].reason);
    let out = PathBuf::from(&report.output_dir).join("hdr.mp4");
    let after = probe(&out);
    assert_eq!(
        field(&after, "color_transfer").as_deref(),
        Some("smpte2084"),
        "默认处理不得改写 HDR 色彩元数据"
    );
    assert_eq!(field(&after, "color_space").as_deref(), Some("bt2020nc"));
    assert_eq!(dims_of(&after), (320, 180));

    // 开启色调映射：输出转为 SDR（bt709）
    let root2 = material_root("s15b");
    fs::copy(&hdr, root2.join("hdr.mp4")).unwrap();
    let files2 = scan_folder(&root2, &ScanOptions::default()).unwrap().files;
    let report2 = run_with(
        &root2,
        &files2,
        Some(Setting::scale(Mode::A, 0.5)),
        VideoOptions {
            tonemap_to_sdr: true,
            ..VideoOptions::default()
        },
    );
    assert_eq!(report2.counts.success, 1, "{:?}", report2.outcomes[0].reason);
    let out2 = PathBuf::from(&report2.output_dir).join("hdr.mp4");
    let after2 = probe(&out2);
    assert_eq!(
        field(&after2, "color_transfer").as_deref(),
        Some("bt709"),
        "色调映射后应转为 SDR"
    );
    assert_eq!(dims_of(&after2), (320, 180));

    let _ = fs::remove_dir_all(root.parent().unwrap());
    let _ = fs::remove_dir_all(root2.parent().unwrap());
}

/// 场景 16：强制使用不可用的硬件编码器，自动回退软件编码并记录回退行为。
#[test]
fn scenario_16_unavailable_hardware_encoder_falls_back_to_software() {
    let root = material_root("s16");
    let source = root.join("clip.mp4");
    ff(&[
        "-hide_banner",
        "-nostdin",
        "-y",
        "-f",
        "lavfi",
        "-i",
        "testsrc=size=640x360:rate=25:duration=2",
        "-c:v",
        "libx264",
        "-crf",
        "30",
        "-preset",
        "ultrafast",
        &source.to_string_lossy(),
    ]);

    // 本机这份 FFmpeg 构建不含任何硬件编码器，强制开启硬件加速必然失败
    let files = scan_folder(&root, &ScanOptions::default()).unwrap().files;
    let report = run_with(
        &root,
        &files,
        Some(Setting::scale(Mode::A, 0.5)),
        VideoOptions {
            codec: "h264".into(),
            hardware: true,
            accel: "nvenc".into(),
            ..VideoOptions::default()
        },
    );

    // 仍然成功输出
    assert_eq!(
        report.counts.success, 1,
        "硬件编码不可用时应回退软件编码而不是失败：{:?}",
        report.outcomes[0].reason
    );

    // 回退行为被记录在运行说明里（界面会把它写进日志）
    assert!(
        report.notes.iter().any(|n| n.contains("回退")),
        "应记录回退行为，实际 notes = {:?}",
        report.notes
    );

    let out = PathBuf::from(&report.output_dir).join("clip.mp4");
    assert_eq!(dims_of(&probe(&out)), (320, 180));
    assert!(can_decode(&out));

    let _ = fs::remove_dir_all(root.parent().unwrap());
}
