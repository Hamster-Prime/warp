//! 在启动时把用户选择的语言应用到 rust-i18n 全局。
//!
//! 调用方式（在 app 启动序列、settings 注册之后）：
//!
//! ```ignore
//! let lang = LanguageSettings::handle(ctx).read().language;
//! i18n::init::apply(lang);
//! ```
//!
//! 之后所有 UI 调用点的 `t!()` 会按 [`rust_i18n::set_locale`] 设置的 locale 查表。

use crate::AppLanguage;

/// 把 [`AppLanguage`] 解析为 BCP-47 并写入 rust-i18n 全局 locale。
///
/// 此函数在用户切换语言时也可调用（见 `appearance_page.rs` 的 `set_language`），
/// rust-i18n 会刷新内部缓存，之后所有 `t!()` 立即按新 locale 查表。
pub fn apply(language: AppLanguage) {
    let locale = language.resolve();
    rust_i18n::set_locale(locale);
}
