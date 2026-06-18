//! Warp 国际化（i18n）入口 crate。
//!
//! 提供：
//! - [`t!`] 宏用于在 UI 调用点查翻译（自包含，消费方无需自行调用 `i18n!`）
//! - [`AppLanguage`] 枚举表示用户可选语言
//! - [`detection::detect_system_language`] 跨平台读取系统语言
//! - [`init::apply`] 在启动时把语言设置应用到 rust-i18n 全局

pub mod detection;
pub mod init;
pub mod language;

pub use language::AppLanguage;

// 在编译期把 `_locales/` 嵌入二进制；找不到 key 时 fallback 到 `"en"`。
//
// 注意：此宏必须在 crate 根（本文件）调用一次，生成的 `_rust_i18n_translate`
// 等 `pub fn` 供下面的 `t!` 宏转发使用。
// （使用 `//` 而非 `///`，因为 rust-i18n 的 `i18n!` 宏展开不接受文档注释。）
rust_i18n::i18n!("_locales", fallback = "en");

// rust_i18n::t! 展开为 `crate::_rust_i18n_t!`，要求调用方 crate 自行调用 `i18n!()`
// 才能生成 `_rust_i18n_t`。为了让下游 crate（如 warp app）直接用 `i18n::t!` 而
// 无需重复 `i18n!()`，这里定义自包含的 `t!` 宏。
//
// 行为对齐 `rust_i18n::_tr!`：查到 → 返回译文；查不到 → 返回 key 本身（而非
// `_rust_i18n_translate` 的 `"{locale}.{key}"` fallback）。
#[doc(hidden)]
pub fn __locale() -> String {
    rust_i18n::locale().to_string()
}

#[doc(hidden)]
pub fn __tr<'a>(locale: &str, key: &'a str) -> std::borrow::Cow<'a, str> {
    match _rust_i18n_try_translate(locale, key) {
        Some(cow) => std::borrow::Cow::Owned(cow.into_owned()),
        None => std::borrow::Cow::Borrowed(key),
    }
}

#[macro_export]
macro_rules! t {
    ($key:expr, locale = $locale:expr $(,)?) => {
        $crate::__tr($locale, $key)
    };
    ($key:expr $(,)?) => {
        $crate::__tr(&$crate::__locale(), $key)
    };
}
