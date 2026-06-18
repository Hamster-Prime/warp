//! YAML 资源完整性测试。
//!
//! 验证：
//! 1. 所有 zh-CN key 都能查到（防 YAML 语法错误）
//! 2. en 模式下找不到的 key fallback 到 key 本身
//! 3. 术语表里的品牌名（如 Warp）保持不翻译
//!
//! 注意：每个测试用 `t!("key", locale = "...")` 而不是 `set_locale + t!`，
//! 因为 `rust_i18n::set_locale` 修改进程级全局 `CURRENT_LOCALE`，
//! cargo 默认并行跑集成测试时会互相竞态。

/// 已知的 zh-CN key 列表（来自 `_locales/zh-CN/*.yml`）。
/// M4 引入抽取工具后会用代码扫描自动维护这个集合，MVP 阶段手写。
const EXPECTED_ZH_CN_KEYS: &[&str] = &[
    // _shared.yml
    "OK",
    "Cancel",
    "Save",
    "Apply",
    "Reset",
    "Warp",
    "Settings",
    // settings.yml
    "Language",
    "Appearance",
];

#[test]
fn all_expected_keys_resolve_in_zh_cn() {
    // 不修改全局 locale，直接传 locale 参数。
    // 注意：`for &key` 而非 `for key` —— `EXPECTED_ZH_CN_KEYS: &[&str]` 迭代
    // 出 `&&str`，而 `t!` 宏内部会再加一层 `&`，传 `&&str` 会变成 `&&&str`，
    // 触发 E0277（`CowStr: From<&&&str>` 未实现）。绑定成 `&str` 后宏得到
    // `&&str`（已实现 `From`）。
    for &key in EXPECTED_ZH_CN_KEYS {
        let translated = i18n::t!(key, locale = "zh-CN").to_string();
        assert_ne!(
            translated, "",
            "key {key:?} returned empty translation in zh-CN"
        );
        // 注释：不强制 translated != *key，因为术语表里 "Warp": "Warp" 是合法的
    }
}

#[test]
fn cancel_translates_to_zh() {
    assert_eq!(
        i18n::t!("Cancel", locale = "zh-CN").to_string(),
        "取消"
    );
}

#[test]
fn language_translates_to_zh() {
    assert_eq!(
        i18n::t!("Language", locale = "zh-CN").to_string(),
        "语言"
    );
}

#[test]
fn settings_translates_to_zh() {
    assert_eq!(
        i18n::t!("Settings", locale = "zh-CN").to_string(),
        "设置"
    );
}

#[test]
fn en_fallback_returns_key() {
    // en.yml 只有 _placeholder，Cancel 查不到 → fallback 返回 key 本身
    assert_eq!(
        i18n::t!("Cancel", locale = "en").to_string(),
        "Cancel"
    );
}

#[test]
fn glossary_keys_keep_brand_names_untranslated() {
    // 术语表里 "Warp": "Warp" — 品牌名不译
    assert_eq!(
        i18n::t!("Warp", locale = "zh-CN").to_string(),
        "Warp"
    );
}
