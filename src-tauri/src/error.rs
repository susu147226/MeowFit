use serde::{Deserialize, Serialize};
use std::fmt;

/// 错误类型枚举，取值与规范第 13.2 条一一对应。
/// 序列化后即为界面与报告上显示的 `E_*` 代码。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorCode {
    #[serde(rename = "E_UNSUPPORTED_FORMAT")]
    UnsupportedFormat,
    #[serde(rename = "E_DECODE_FAILED")]
    DecodeFailed,
    #[serde(rename = "E_SIZE_UNDERFLOW")]
    SizeUnderflow,
    #[serde(rename = "E_SIZE_OVERFLOW")]
    SizeOverflow,
    #[serde(rename = "E_INCOMPLETE_DIMENSION")]
    IncompleteDimension,
    #[serde(rename = "E_NO_SPACE")]
    NoSpace,
    #[serde(rename = "E_OUTPUT_UNWRITABLE")]
    OutputUnwritable,
    #[serde(rename = "E_FFMPEG_MISSING")]
    FfmpegMissing,
    #[serde(rename = "E_FFMPEG_ENCODE_FAILED")]
    FfmpegEncodeFailed,
    #[serde(rename = "E_OUTPUT_VERIFY_FAILED")]
    OutputVerifyFailed,
    #[serde(rename = "E_WRITE_FAILED")]
    WriteFailed,
    #[serde(rename = "E_TARGET_UNREACHABLE")]
    TargetUnreachable,
}

impl ErrorCode {
    /// 对外暴露的代码字符串，与 13.2 表格中的写法一致。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UnsupportedFormat => "E_UNSUPPORTED_FORMAT",
            Self::DecodeFailed => "E_DECODE_FAILED",
            Self::SizeUnderflow => "E_SIZE_UNDERFLOW",
            Self::SizeOverflow => "E_SIZE_OVERFLOW",
            Self::IncompleteDimension => "E_INCOMPLETE_DIMENSION",
            Self::NoSpace => "E_NO_SPACE",
            Self::OutputUnwritable => "E_OUTPUT_UNWRITABLE",
            Self::FfmpegMissing => "E_FFMPEG_MISSING",
            Self::FfmpegEncodeFailed => "E_FFMPEG_ENCODE_FAILED",
            Self::OutputVerifyFailed => "E_OUTPUT_VERIFY_FAILED",
            Self::WriteFailed => "E_WRITE_FAILED",
            Self::TargetUnreachable => "E_TARGET_UNREACHABLE",
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// 带代码的中文错误。`detail` 用于补足「缺哪一边」「超出多少」这类具体信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: ErrorCode,
    pub message: String,
}

impl AppError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn size_underflow() -> Self {
        Self::new(ErrorCode::SizeUnderflow, "计算结果小于 1px，请修正尺寸或倍率")
    }

    pub fn size_overflow() -> Self {
        Self::new(ErrorCode::SizeOverflow, "超出尺寸上限 32768")
    }

    pub fn incomplete_dimension(missing_width: bool) -> Self {
        let which = if missing_width { "缺少宽度" } else { "缺少高度" };
        Self::new(
            ErrorCode::IncompleteDimension,
            format!("{which}：已关闭按比例自动计算，宽高必须都填写"),
        )
    }

    pub fn unsupported_format(detail: impl Into<String>) -> Self {
        Self::new(ErrorCode::UnsupportedFormat, detail)
    }

    pub fn decode_failed(detail: impl Into<String>) -> Self {
        Self::new(ErrorCode::DecodeFailed, detail)
    }

    pub fn write_failed(detail: impl Into<String>) -> Self {
        Self::new(ErrorCode::WriteFailed, detail)
    }

    pub fn output_unwritable(detail: impl Into<String>) -> Self {
        Self::new(ErrorCode::OutputUnwritable, detail)
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.code.as_str(), self.message)
    }
}

impl std::error::Error for AppError {}

pub type AppResult<T> = Result<T, AppError>;
