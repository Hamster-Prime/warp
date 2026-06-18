//! 系统语言检测。
//!
//! 把跨平台 OS API（`sys_locale::get_locale()`）返回的多种格式统一归一化到
//! Warp 内部使用的 BCP-47 代码（目前仅 `"zh-CN"` 与 `"en"`）。
//!
//! 归一化结果在进程生命周期内通过 [`DETECTED`] 缓存一次，
//! 仅当用户设置选择 `System` 时才会触发首次检测。

use std::sync::OnceLock;

static DETECTED: OnceLock<&'static str> = OnceLock::new();

/// 读取系统语言并归一化。结果缓存。
///
/// 返回值是 rust-i18n 能识别的 locale 字符串：
/// - 中文系统（含简/繁）→ `"zh-CN"`
/// - 其他系统 → `"en"`
pub fn detect_system_language() -> &'static str {
    *DETECTED.get_or_init(|| {
        let raw = sys_locale::get_locale().unwrap_or_else(|| "en".to_string());
        normalize(&raw).unwrap_or("en")
    })
}

/// 把各种 locale 字符串归一化为 Warp 内部使用的 BCP-47 代码。
///
/// 支持的输入格式：
/// - POSIX: `zh_CN.UTF-8`, `zh_CN`, `en_US`
/// - macOS NSLocale: `zh-Hans-CN`, `zh-Hant-TW`
/// - Windows / 通用 BCP-47: `zh-CN`, `en-US`
///
/// 返回 `None` 表示输入为空字符串，调用方应 fallback 到 `"en"`。
pub fn normalize(raw: &str) -> Option<&'static str> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let lower = trimmed.to_lowercase();
    if lower.starts_with("zh") {
        // MVP：所有中文变体（简/繁/区域）都归一到 zh-CN
        Some("zh-CN")
    } else {
        // 其他语言一律 fallback 到英文（MVP 范围只覆盖中英）
        Some("en")
    }
}
