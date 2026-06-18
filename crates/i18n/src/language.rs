//! 用户可选的应用语言枚举。
//!
//! 作为 `LanguageSettings`（在 app crate 中）的值类型，
//! 通过 [`settings_value::SettingsValue`] 派生宏接入 Warp 的 settings 系统，
//! 序列化为 snake_case（参考 `app/src/settings/mod.rs:460-484` 的 `EnforceMinimumContrast`）。

use serde::{Deserialize, Serialize};

/// 用户可选的应用界面语言。
///
/// 顺序决定下拉项的展示顺序（参考 [`AppLanguage::all`]）。
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
    settings_value::SettingsValue,
)]
#[serde(rename_all = "snake_case")]
#[schemars(
    description = "The display language of the Warp UI.",
    rename_all = "snake_case"
)]
pub enum AppLanguage {
    /// 跟随系统语言（启动时检测一次）。
    #[default]
    System,
    /// 强制英文。
    English,
    /// 强制简体中文。
    SimplifiedChinese,
}

impl AppLanguage {
    /// 下拉项的可选值列表（顺序即 UI 展示顺序）。
    pub fn all() -> &'static [AppLanguage] {
        &[Self::System, Self::English, Self::SimplifiedChinese]
    }

    /// 下拉项的显示文案。
    ///
    /// 约定：非英文选项用目标语言的母语写法显示，
    /// 方便用户在英文 UI 下也能识别自己的语言。
    pub fn dropdown_label(&self) -> &'static str {
        match self {
            Self::System => "System",
            Self::English => "English",
            Self::SimplifiedChinese => "简体中文",
        }
    }

    /// 解析为 rust-i18n 能识别的 BCP-47 语言代码。
    ///
    /// - [`AppLanguage::System`] 会触发 [`crate::detection::detect_system_language`]，
    ///   返回值在 MVP 中只可能是 `"zh-CN"` 或 `"en"`。
    /// - [`AppLanguage::English`] / [`AppLanguage::SimplifiedChinese`] 返回固定值。
    pub fn resolve(self) -> &'static str {
        match self {
            Self::System => crate::detection::detect_system_language(),
            Self::English => "en",
            Self::SimplifiedChinese => "zh-CN",
        }
    }
}
