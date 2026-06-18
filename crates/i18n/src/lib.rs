//! Warp 国际化（i18n）入口 crate。
//!
//! 提供：
//! - [`t!`] 宏（re-export 自 rust-i18n）用于在 UI 调用点查翻译
//! - [`AppLanguage`] 枚举表示用户可选语言
//! - [`detection::detect_system_language`] 跨平台读取系统语言
//! - [`init::apply`] 在启动时把语言设置应用到 rust-i18n 全局

pub use rust_i18n::t;

pub mod detection;
pub mod init;
pub mod language;

pub use language::AppLanguage;

// 在编译期把 `_locales/` 嵌入二进制；找不到 key 时 fallback 到 `"en"`。
//
// 注意：此宏必须在 crate 根（本文件）调用一次，之后所有 `t!()` 才能查到表。
// （使用 `//` 而非 `///`，因为 rust-i18n 的 `i18n!` 宏展开不接受文档注释。）
rust_i18n::i18n!("_locales", fallback = "en");
