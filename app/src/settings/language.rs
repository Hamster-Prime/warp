//! User interface language setting.
//!
//! Modeled after `app/src/settings/font.rs`' `FontSettings` template.
//! The single field `language` is of type [`i18n::AppLanguage`] (which
//! derives `settings_value::SettingsValue` and so plugs into the settings
//! system).
//!
//! Persisted to the `[appearance].language` path in `settings.toml`.
//! Defaults to [`i18n::AppLanguage::System`] (follow the system language).
//!
//! The macro auto-generates and re-exports `LanguageSettingsChangedEvent`
//! from this module, fired whenever `LanguageSettings::language` changes.
//! Subscribe using the same pattern as `FontSettingsChangedEvent`
//! (called from the Appearance settings page when the user changes
//! the Language dropdown):
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

/// Convenience helper: reads the current language (hides the settings group indirection).
pub fn current_language(ctx: &AppContext) -> AppLanguage {
    *LanguageSettings::as_ref(ctx).language
}

/// Writes the current language. Returns `Ok(())` on success.
///
/// The caller is responsible for afterwards calling
/// `i18n::init::apply(new_language)` to switch the rust-i18n global locale
/// (called from the Appearance settings page when the user changes
/// the Language dropdown; see future `appearance_page.rs::set_language`).
pub fn set_app_language(value: AppLanguage, ctx: &mut AppContext) -> anyhow::Result<()> {
    LanguageSettings::handle(ctx)
        .update(ctx, |settings, ctx| settings.language.set_value(value, ctx))
}
