//! 图片元数据：ICC 色彩配置、EXIF、XMP 的读取与回写（规范 6.8）。
//!
//! 处理原则：
//! - **ICC 必须保留**（6.8），否则缩放后颜色会与原图不一致。
//! - EXIF / XMP 保留基础信息（拍摄参数、版权），供元数据开关的两态使用。
//! - 像素已按 EXIF 方向旋转为正向，因此回写时必须**清除方向标签**，
//!   否则正确的看图软件会再旋转一次（规范 6.8「先按 EXIF 旋转再缩放」）。

use std::path::Path;

use image::metadata::Orientation;
use img_parts::jpeg::{markers, JpegSegment};
use img_parts::{Bytes, DynImage, ImageEXIF, ImageICC};

use crate::error::{AppError, ErrorCode};

/// APP1 段中 XMP 的标识前缀
const XMP_SIGNATURE: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";
/// APP1 段中扩展 XMP 的标识前缀
const EXTENDED_XMP_SIGNATURE: &[u8] = b"http://ns.adobe.com/xmp/extension/\0";

#[derive(Debug, Clone, Default)]
pub struct SourceMeta {
    pub icc: Option<Vec<u8>>,
    pub exif: Option<Vec<u8>>,
    pub xmp: Vec<Vec<u8>>,
    /// 其余 APPn 段（marker, 载荷）。默认被剥离；开启「保留全部元数据」时原样写回。
    pub extraneous: Vec<(u8, Vec<u8>)>,
}

impl SourceMeta {
    /// EXIF 中的方向标签；无 EXIF 或未设置时为 `NoTransforms`。
    pub fn orientation(&self) -> Orientation {
        let Some(raw) = &self.exif else {
            return Orientation::NoTransforms;
        };
        // `from_exif_chunk` 接受的是 TIFF 头开始的部分，需要去掉 "Exif\0\0" 前缀
        let tiff = raw.strip_prefix(b"Exif\0\0").unwrap_or(raw);
        Orientation::from_exif_chunk(tiff).unwrap_or(Orientation::NoTransforms)
    }

    /// 该方向是否需要交换宽高（旋转 90 / 270 度及其镜像）。
    pub fn swaps_axes(&self) -> bool {
        matches!(
            self.orientation(),
            Orientation::Rotate90
                | Orientation::Rotate270
                | Orientation::Rotate90FlipH
                | Orientation::Rotate270FlipH
        )
    }

    /// 回写用的 EXIF：清除方向标签，其余原样保留。
    fn exif_for_output(&self) -> Option<Vec<u8>> {
        let raw = self.exif.as_ref()?;
        let mut out = raw.clone();
        let offset = if out.starts_with(b"Exif\0\0") { 6 } else { 0 };
        if offset < out.len() {
            // 清除方向标签：像素已按该方向旋转，保留会让正确的看图软件再转一次
            let _ = Orientation::remove_from_exif_chunk(&mut out[offset..]);
        }
        Some(out)
    }

    /// 是否含有任何可回写的元数据。
    pub fn is_empty(&self) -> bool {
        self.icc.is_none() && self.exif.is_none() && self.xmp.is_empty()
    }
}

fn is_xmp(payload: &[u8]) -> bool {
    payload.starts_with(XMP_SIGNATURE) || payload.starts_with(EXTENDED_XMP_SIGNATURE)
}

/// 读取源文件的元数据。读不到（无元数据、格式不支持解析）时返回空值，不报错。
pub fn read(path: &Path) -> SourceMeta {
    let Ok(bytes) = std::fs::read(path) else {
        return SourceMeta::default();
    };
    let Ok(Some(image)) = DynImage::from_bytes(Bytes::from(bytes)) else {
        return SourceMeta::default();
    };

    let icc = image.icc_profile().map(|b| b.to_vec());
    let exif = image.exif().map(|b| b.to_vec());

    // XMP 与其余 APPn 只在 JPEG 上做段级处理；PNG / WebP 的基础元数据已由上面的 trait 覆盖
    let mut xmp = Vec::new();
    let mut extraneous = Vec::new();
    if let DynImage::Jpeg(jpeg) = &image {
        for segment in jpeg.segments() {
            let marker = segment.marker();
            // SOI 与熵编码数据段不属于 APPn
            if marker != markers::APP1 && !(markers::APP0..=markers::APP15).contains(&marker) {
                continue;
            }
            let contents = segment.contents();
            if marker == markers::APP1 {
                if is_xmp(contents) {
                    xmp.push(contents.to_vec());
                    continue;
                }
                // EXIF 段已由 `exif()` 取出，不重复收集
                if contents.starts_with(b"Exif\0\0") {
                    continue;
                }
            }
            // ICC 段由 `icc_profile()` 合并取出，不重复收集
            if contents.starts_with(b"ICC_PROFILE\0") {
                continue;
            }
            extraneous.push((marker, contents.to_vec()));
        }
    }

    SourceMeta {
        icc,
        exif,
        xmp,
        extraneous,
    }
}

/// 把元数据写入已编码好的图片字节，并落盘。返回文件字节数。
///
/// `keep_all == false`（默认）时只保留 ICC + EXIF + XMP，剥离缩略图等冗余段。
pub fn write_with_metadata(
    out: &Path,
    encoded: Vec<u8>,
    meta: &SourceMeta,
    keep_all: bool,
) -> Result<u64, AppError> {
    if meta.is_empty() {
        std::fs::write(out, &encoded)
            .map_err(|e| AppError::write_failed(format!("写出 {} 失败：{e}", out.display())))?;
        return Ok(encoded.len() as u64);
    }

    let mut image = DynImage::from_bytes(Bytes::from(encoded))
        .map_err(|e| AppError::new(ErrorCode::WriteFailed, format!("无法解析刚编码的输出：{e}")))?
        .ok_or_else(|| {
            AppError::new(ErrorCode::WriteFailed, "无法识别刚编码输出的容器格式".to_string())
        })?;

    if let Some(icc) = &meta.icc {
        image.set_icc_profile(Some(Bytes::copy_from_slice(icc)));
    }
    if let Some(exif) = meta.exif_for_output() {
        image.set_exif(Some(Bytes::from(exif)));
    }

    if let DynImage::Jpeg(jpeg) = &mut image {
        for payload in &meta.xmp {
            jpeg.segments_mut()
                .insert(3, JpegSegment::new_with_contents(markers::APP1, Bytes::copy_from_slice(payload)));
        }
        if keep_all {
            // 「保留全部元数据」：把源文件里其余的 APPn 段原样插回（需插在 SOF/SOS 之前）
            for (marker, payload) in &meta.extraneous {
                jpeg.segments_mut().insert(
                    3,
                    JpegSegment::new_with_contents(*marker, Bytes::copy_from_slice(payload)),
                );
            }
        }
    }

    let file = std::fs::File::create(out)
        .map_err(|e| AppError::write_failed(format!("创建 {} 失败：{e}", out.display())))?;
    let mut writer = std::io::BufWriter::new(file);
    image
        .encoder()
        .write_to(&mut writer)
        .map_err(|e| AppError::write_failed(format!("写出 {} 失败：{e}", out.display())))?;
    std::io::Write::flush(&mut writer)
        .map_err(|e| AppError::write_failed(format!("写出 {} 失败：{e}", out.display())))?;

    std::fs::metadata(out)
        .map(|m| m.len())
        .map_err(|e| AppError::write_failed(format!("读取输出文件信息失败：{e}")))
}
/// 供扫描阶段读取方向：返回是否应交换宽高。
pub fn swaps_axes(path: &Path) -> bool {
    read(path).swaps_axes()
}
