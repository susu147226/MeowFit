//! 核心算法。全部为纯函数，前端预览与后端执行共用同一套实现（规范第十章）。
//! 本模块的任何函数都不得依赖文件系统、不得依赖 Tauri。

pub mod grouping;
pub mod plan;
pub mod size;
