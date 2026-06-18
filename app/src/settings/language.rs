//! 用户界面语言设置。
//!
//! 仿照 `app/src/settings/font.rs:15-113` 的 `FontSettings` 模板。
//! 唯一字段 `language` 类型是 [`i18n::AppLanguage`]（已通过
//! `settings_value::SettingsValue` 派生宏接入 settings 系统）。
//!
//! 持久化到 `settings.toml` 的 `[appearance].language` 路径
//! （参考 `app/src/settings/mod.rs:597`）。
//! 默认值 [`i18n::AppLanguage::System`] 跟随系统语言。
//!
//! 宏会自动生成并在本模块导出 `LanguageSettingsChangedEvent`，当
//! `LanguageSettings::language` 变化时触发。订阅方式（参照
//! `appearance_page.rs` 中 `FontSettingsChangedEvent` 的模式）：
//! ```ignore
//! ctx.observe(&LanguageSettings::handle(ctx).entity_id(), |_, ctx| { ... })
//! ```

use i18n::AppLanguage;
use settings::macros::define_settings_group;
use settings::{RespectUserSyncSetting, Setting, SupportedPlatforms, SyncToCloud};
use warpui::{AppContext, SingletonEntity};

define_settings_group!(
    LanguageSettings,
    settings: [
        language: Language {
            type: AppLanguage,
            default: AppLanguage::default(),
            supported_platforms: SupportedPlatforms::ALL,
            sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
            private: false,
            storage_key: "Language",
            toml_path: "appearance.language",
            description: "The display language of the Warp UI.",
        },
    ]
);

/// 便利方法：读取当前语言（隐藏 settings group 的间接层）。
pub fn current_language(ctx: &AppContext) -> AppLanguage {
    *LanguageSettings::as_ref(ctx).language
}

/// 便利方法：写入当前语言，返回是否成功。
///
/// 调用者负责后续调用 `i18n::init::apply(new_language)` 切换 rust-i18n 全局 locale
/// （见 `appearance_page.rs::set_language`）。
pub fn set_language(value: AppLanguage, ctx: &mut AppContext) -> anyhow::Result<()> {
    LanguageSettings::handle(ctx).update(ctx, |settings, ctx| {
        settings.language.set_value(value, ctx)
    })
}
