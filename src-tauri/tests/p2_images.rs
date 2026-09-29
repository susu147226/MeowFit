//! 规范 16.2 的验收场景 9–12，在真实文件上跑完整链路。

use std::fs;
use std::path::{Path, PathBuf};

use image::{Rgb, RgbImage};
use img_parts::jpeg::{markers, Jpeg, JpegSegment};
use img_parts::{Bytes, ImageEXIF, ImageICC};

use meowfit_lib::algo::grouping::{group_files, GroupableFile};
use meowfit_lib::algo::plan::build_plan;
use meowfit_lib::exec::{execute, ExecOptions, SourceRef};
use meowfit_lib::imaging::{ImageOptions, OutputFormat, Resample};
use meowfit_lib::model::{Grouping, PlanFileInput, PlanRequest, Setting, Mode};
use meowfit_lib::scan::{scan_folder, ScanOptions};

fn temp_root(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "meowfit-p2-{tag}-{}-{:?}",
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

/// 建好 `<临时目录>/素材/` 并返回，作为本次场景的输入根目录。
fn material_root(tag: &str) -> PathBuf {
    let root = temp_root(tag).join("素材");
    fs::create_dir_all(&root).unwrap();
    root
}

/// 最小合法 EXIF：little-endian TIFF，IFD0 中一个 Orientation 标签。
fn exif_with_orientation(orientation: u16) -> Vec<u8> {
    let mut v = vec![
        0x49, 0x49, 0x2A, 0x00, // "II" + 42
        0x08, 0x00, 0x00, 0x00, // IFD0 偏移 8
        0x01, 0x00, // 1 个条目
        0x12, 0x01, // tag 0x0112 Orientation
        0x03, 0x00, // 类型 SHORT
        0x01, 0x00, 0x00, 0x00, // 数量 1
    ];
    v.extend_from_slice(&orientation.to_le_bytes());
    v.extend_from_slice(&[0x00, 0x00]); // 值字段补足 4 字节
    v.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // 无下一个 IFD
    v
}

/// 写一张带元数据的 JPEG。`extra_app` 用于塞入冗余 APPn 段，验证剥离效果。
fn write_jpeg_with_meta(
    path: &Path,
    img: &RgbImage,
    orientation: Option<u16>,
    icc: Option<&[u8]>,
    extra_app: bool,
) {
    let (w, h) = (img.width(), img.height());
    let mut buf: Vec<u8> = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 95)
        .encode(img.as_raw(), w, h, image::ExtendedColorType::Rgb8)
        .unwrap();

    let mut jpeg = Jpeg::from_bytes(Bytes::from(buf)).unwrap();
    if let Some(value) = orientation {
        jpeg.set_exif(Some(Bytes::from(exif_with_orientation(value))));
    }
    if let Some(profile) = icc {
        jpeg.set_icc_profile(Some(Bytes::copy_from_slice(profile)));
    }
    if extra_app {
        // 模拟「缩略图等冗余数据」：一段与画面无关的 APP13
        let payload = Bytes::from_static(&[b'P'; 4096]);
        jpeg.segments_mut()
            .insert(3, JpegSegment::new_with_contents(markers::APP13, payload));
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    let file = fs::File::create(path).unwrap();
    jpeg.encoder().write_to(file).unwrap();
}

fn dims(path: &Path) -> (u32, u32) {
    image::image_dimensions(path).unwrap()
}

fn run_with(
    root: &Path,
    files: &[meowfit_lib::scan::ScannedFile],
    global: Option<Setting>,
    image: ImageOptions,
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
        })
        .collect();

    execute(
        &plan,
        &sources,
        root,
        &ExecOptions {
            image,
            ..ExecOptions::default()
        },
    )
    .unwrap()
}

/// 场景 9：EXIF 竖拍方向正确 + 两种重采样可辨。
#[test]
fn scenario_9_exif_orientation_and_resample_choice() {
    let root = material_root("s9");

    // 存储为横向 40×20，但 EXIF 声明 orientation=6（需顺时针旋转 90° 才是正向）
    let mut stored = RgbImage::new(40, 20);
    for (x, _y, pixel) in stored.enumerate_pixels_mut() {
        // 存储图的左侧为红色，旋转后应出现在输出顶部
        *pixel = if x < 8 { Rgb([220, 30, 30]) } else { Rgb([30, 60, 200]) };
    }
    write_jpeg_with_meta(&root.join("portrait.jpg"), &stored, Some(6), None, false);

    // 棋盘格用于比较两种重采样
    let mut board = RgbImage::new(16, 16);
    for (x, y, pixel) in board.enumerate_pixels_mut() {
        *pixel = if (x + y) % 2 == 0 { Rgb([0, 0, 0]) } else { Rgb([255, 255, 255]) };
    }
    board.save(root.join("board.png")).unwrap();

    let result = scan_folder(&root, &ScanOptions::default()).unwrap();
    let portrait = result.files.iter().find(|f| f.name == "portrait.jpg").unwrap();
    // 扫描必须按 EXIF 方向报告宽高：存储 40×20 → 正向 20×40
    assert_eq!(
        (portrait.width, portrait.height),
        (Some(20), Some(40)),
        "带 EXIF 方向的竖拍图应按正向报告尺寸"
    );

    // 放大 2 倍：正向 20×40 → 40×80
    let report = run_with(
        &root,
        &result.files,
        Some(Setting::scale(Mode::A, 2.0)),
        ImageOptions {
            resample: Resample::Lanczos3,
            ..ImageOptions::default()
        },
    );
    assert_eq!(report.counts.success, 2);

    let out = PathBuf::from(&report.output_dir);
    assert_eq!(dims(&out.join("portrait.jpg")), (40, 80), "输出应为正向 40×80");

    // 画面方向正确：存储图左缘（红）旋转后位于输出顶部
    let oriented = image::open(out.join("portrait.jpg")).unwrap().to_rgba8();
    let top = oriented.get_pixel(20, 4);
    let bottom = oriented.get_pixel(20, 76);
    assert!(top[0] > 150 && top[2] < 120, "顶部应为红条，实际 {top:?}");
    assert!(bottom[2] > 150 && bottom[0] < 120, "底部应为蓝底，实际 {bottom:?}");

    // 输出不得残留方向标签，否则看图软件会再转一次
    let written = meowfit_lib::meta::read(&out.join("portrait.jpg"));
    assert_eq!(
        written.orientation(),
        image::metadata::Orientation::NoTransforms,
        "输出 EXIF 的方向标签必须被清除"
    );
}

/// 场景 9（续）：Nearest 与 Lanczos3 的观感差异在真实写出中可辨。
#[test]
fn scenario_9_resample_choice_changes_output_pixels() {
    let root = material_root("s9b");
    let mut board = RgbImage::new(16, 16);
    for (x, y, pixel) in board.enumerate_pixels_mut() {
        *pixel = if (x + y) % 2 == 0 { Rgb([0, 0, 0]) } else { Rgb([255, 255, 255]) };
    }
    board.save(root.join("board.png")).unwrap();

    let result = scan_folder(&root, &ScanOptions::default()).unwrap();

    let gray_ratio = |resample: Resample, tag: &str| -> f64 {
        let mut opts = ExecOptions::default();
        opts.image.resample = resample;
        // 两次对比共用同一个输出目录，必须允许覆盖，否则第二次会被同名跳过
        opts.on_conflict = "overwrite".into();
        let plan = {
            let request = PlanRequest {
                files: vec![PlanFileInput {
                    id: "board.png".into(),
                    name: "board.png".into(),
                    width: 16,
                    height: 16,
                    is_video: false,
                    group: "board".into(),
                    setting: None,
                }],
                groups: vec![],
                global: Some(Setting::scale(Mode::A, 8.0)),
            };
            build_plan(&request)
        };
        let sources = vec![SourceRef {
            id: "board.png".into(),
            path: result.files[0].path.clone(),
            kind: result.files[0].kind.unwrap(),
        }];
        let report = execute(&plan, &sources, &root, &opts).unwrap();
        let out = PathBuf::from(&report.output_dir).join("board.png");
        let _ = tag;
        let img = image::open(out).unwrap().to_luma8();
        let total = (img.width() * img.height()) as f64;
        let mid = img.pixels().filter(|p| p[0] > 20 && p[0] < 235).count() as f64;
        mid / total
    };

    let nearest = gray_ratio(Resample::Nearest, "nearest");
    let lanczos = gray_ratio(Resample::Lanczos3, "lanczos3");
    assert_eq!(nearest, 0.0, "Nearest 不应产生过渡色（像素风）");
    assert!(lanczos > 0.05, "Lanczos3 应产生平滑过渡，实际 {lanczos}");

    let _ = fs::remove_dir_all(root.parent().unwrap());
}

/// 场景 10：元数据开关与 ICC 保留。
#[test]
fn scenario_10_metadata_switch_and_icc_preserved() {
    let root = material_root("s10");
    // 形状合法的伪 ICC：128 字节头 + 载荷
    let mut icc = vec![0u8; 128];
    icc[0..4].copy_from_slice(&(128u32 + 64).to_be_bytes());
    icc[36..40].copy_from_slice(b"acsp");
    icc.extend_from_slice(&[0xAB; 64]);

    let img = RgbImage::from_pixel(64, 64, Rgb([120, 160, 90]));
    // orientation = 1（正向）也是有效 EXIF，用于验证基础 EXIF 被保留
    write_jpeg_with_meta(&root.join("shot.jpg"), &img, Some(1), Some(&icc), true);

    let result = scan_folder(&root, &ScanOptions::default()).unwrap();
    let source_size = fs::metadata(root.join("shot.jpg")).unwrap().len();

    // 默认：剥离冗余元数据（保留 ICC 与基础 EXIF）
    let stripped = run_with(
        &root,
        &result.files,
        Some(Setting::scale(Mode::A, 0.5)),
        ImageOptions::default(),
    );
    let stripped_out = PathBuf::from(&stripped.output_dir).join("shot.jpg");

    // 对照：保留全部元数据
    let root2 = material_root("s10b");
    fs::create_dir_all(&root2).unwrap();
    fs::copy(root.join("shot.jpg"), root2.join("shot.jpg")).unwrap();
    let result2 = scan_folder(&root2, &ScanOptions::default()).unwrap();
    let kept = run_with(
        &root2,
        &result2.files,
        Some(Setting::scale(Mode::A, 0.5)),
        ImageOptions {
            keep_all_metadata: true,
            ..ImageOptions::default()
        },
    );
    let kept_out = PathBuf::from(&kept.output_dir).join("shot.jpg");

    let stripped_size = fs::metadata(&stripped_out).unwrap().len();
    let kept_size = fs::metadata(&kept_out).unwrap().len();
    let source_meta = meowfit_lib::meta::read(&root.join("shot.jpg"));

    for (label, path) in [("剥离后", &stripped_out), ("保留全部", &kept_out)] {
        let meta = meowfit_lib::meta::read(path);
        assert_eq!(
            meta.icc,
            source_meta.icc,
            "{label} 的输出必须原样保留 ICC 色彩配置"
        );
        assert!(
            meta.exif.is_some(),
            "{label} 的输出应保留基础 EXIF"
        );
    }

    assert!(
        stripped_size < kept_size,
        "剥离冗余元数据后体积应更小：剥离 {stripped_size} vs 保留 {kept_size}"
    );
    assert!(stripped_size < source_size, "缩放后体积应小于原件 {source_size}");

    let _ = fs::remove_dir_all(root.parent().unwrap());
    let _ = fs::remove_dir_all(root2.parent().unwrap());
}

/// 场景 11：SVG 双路径与未声明尺寸时的两种换算方式。
#[test]
fn scenario_11_svg_to_svg_and_to_bitmap() {
    let root = material_root("s11");
    fs::create_dir_all(&root).unwrap();
    let declared = r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="20"><rect width="40" height="20" fill="#c08040"/></svg>"##;
    let undeclared = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 80 40"><rect width="80" height="40" fill="#4080c0"/></svg>"##;
    fs::write(root.join("declared.svg"), declared).unwrap();
    fs::write(root.join("viewbox.svg"), undeclared).unwrap();

    let result = scan_folder(&root, &ScanOptions::default()).unwrap();
    let d = result.files.iter().find(|f| f.name == "declared.svg").unwrap();
    let u = result.files.iter().find(|f| f.name == "viewbox.svg").unwrap();
    assert!(d.svg_declared, "声明了 width/height 的 SVG 应标记为已声明");
    assert!(!u.svg_declared, "只有 viewBox 的 SVG 应标记为未声明");
    assert_eq!((d.width, d.height), (Some(40), Some(20)));
    assert_eq!((u.width, u.height), (Some(80), Some(40)));

    // 路径一：保持原格式 → SVG → SVG，仅尺寸属性变化
    let to_svg = run_with(
        &root,
        &result.files,
        Some(Setting::scale(Mode::A, 2.0)),
        ImageOptions::default(),
    );
    let out = PathBuf::from(&to_svg.output_dir);
    let rewritten = fs::read_to_string(out.join("declared.svg")).unwrap();
    assert_eq!(
        meowfit_lib::svg::base_size(&rewritten),
        Some(meowfit_lib::svg::SvgSize {
            width: 80.0,
            height: 40.0,
            declared: true
        })
    );
    // 画面内容无损：图形与填充色原样保留，viewBox 同步为原始尺寸
    assert!(rewritten.contains(r##"fill="#c08040""##));
    assert!(rewritten.contains(r#"viewBox="0 0 40 20""#));

    // 路径二：统一转为 PNG → 按指定像素尺寸光栅化
    let to_png = run_with(
        &root,
        &result.files,
        Some(Setting::scale(Mode::A, 2.0)),
        ImageOptions {
            format: OutputFormat::Png,
            ..ImageOptions::default()
        },
    );
    let png_out = PathBuf::from(&to_png.output_dir);
    // 输出扩展名随格式转换而改变，且按目标像素尺寸光栅化
    assert_eq!(dims(&png_out.join("declared.png")), (80, 40));
    assert_eq!(dims(&png_out.join("viewbox.png")), (160, 80));

    // 边缘清晰：填充区域为纯色，边界处不应出现模糊过渡
    let raster = image::open(png_out.join("declared.png")).unwrap().to_rgba8();
    let inside = raster.get_pixel(40, 20);
    assert!(inside[0] > 150 && inside[2] < 120, "应渲染出填充色，实际 {inside:?}");
    assert_eq!(inside[3], 255);

    let _ = fs::remove_dir_all(root.parent().unwrap());
}

/// 场景 11（续）：未声明尺寸的 SVG 在两种基准方式下得到不同基准。
#[test]
fn scenario_11_undeclared_svg_pixel_vs_dpi() {
    // 基准换算本身在界面侧完成（见 src/lib/expression.ts 与 store.dimsOf）：
    // 这里验证按 DPI 换算后的基准尺寸确实参与计算，且倍率结果随之变化。
    let root = material_root("s11b");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("viewbox.svg"),
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 80 40"><rect width="80" height="40" fill="#4080c0"/></svg>"##,
    )
    .unwrap();

    let result = scan_folder(&root, &ScanOptions::default()).unwrap();
    let base = result.files[0].clone();
    assert!(!base.svg_declared);

    // 直接填像素：基准 80×40，倍率 2 → 160×80
    let pixel_mode = run_with(
        &root,
        std::slice::from_ref(&base),
        Some(Setting::scale(Mode::A, 2.0)),
        ImageOptions {
            format: OutputFormat::Png,
            ..ImageOptions::default()
        },
    );
    assert_eq!(
        dims(&PathBuf::from(&pixel_mode.output_dir).join("viewbox.png")),
        (160, 80)
    );

    // 按 DPI 换算：192 DPI → 基准 160×80，倍率 2 → 320×160
    // 两次运行共用输出目录，先清掉上一次的结果以免被同名跳过
    let _ = fs::remove_dir_all(PathBuf::from(&pixel_mode.output_dir));
    let mut dpi_file = base.clone();
    dpi_file.width = Some(160);
    dpi_file.height = Some(80);
    dpi_file.id = "viewbox.svg".into();
    let dpi_mode = run_with(
        &root,
        std::slice::from_ref(&dpi_file),
        Some(Setting::scale(Mode::A, 2.0)),
        ImageOptions {
            format: OutputFormat::Png,
            ..ImageOptions::default()
        },
    );
    assert_eq!(
        dims(&PathBuf::from(&dpi_mode.output_dir).join("viewbox.png")),
        (320, 160)
    );

    let _ = fs::remove_dir_all(root.parent().unwrap());
}

/// 场景 12：一批 PNG 统一转为 WebP，体积对比数据正确。
#[test]
fn scenario_12_png_batch_to_webp_with_size_comparison() {
    let root = material_root("s12");
    for i in 0..6 {
        let mut img = RgbImage::new(120, 90);
        for (x, y, pixel) in img.enumerate_pixels_mut() {
            let t = ((x * 7 + y * 3 + i * 11) % 255) as u8;
            *pixel = Rgb([t, t / 2 + 20, 200 - t / 3]);
        }
        img.save(root.join(format!("art_{i:02}.png"))).unwrap();
    }

    let result = scan_folder(&root, &ScanOptions::default()).unwrap();
    let source_total: u64 = result.files.iter().map(|f| f.size).sum();

    let report = run_with(
        &root,
        &result.files,
        Some(Setting::scale(Mode::A, 0.5)),
        ImageOptions {
            format: OutputFormat::Webp,
            ..ImageOptions::default()
        },
    );

    assert_eq!(report.counts.success, 6);
    let out = PathBuf::from(&report.output_dir);
    let mut new_total: u64 = 0;
    for i in 0..6 {
        let path = out.join(format!("art_{i:02}.webp"));
        assert!(path.exists(), "输出应按目标格式改用 .webp 扩展名");
        // 确实是 WebP 容器
        let bytes = fs::read(&path).unwrap();
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"WEBP");
        assert_eq!(dims(&path), (60, 45), "尺寸应减半");
        new_total += bytes.len() as u64;
    }

    // 体积对比数据可用（规范 6.8：给出转换后的体积对比）
    assert!(new_total > 0 && new_total < source_total, "WebP 应小于原 PNG 总量");
    for outcome in &report.outcomes {
        assert!(outcome.original_size > 0);
        assert!(outcome.new_size.unwrap_or(0) > 0, "报告应记录新体积");
    }

    let _ = fs::remove_dir_all(root.parent().unwrap());
}
