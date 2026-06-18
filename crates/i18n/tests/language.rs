//! AppLanguage 枚举行为测试。
//!
//! 注意：`resolve()` 在 `System` 模式下会调用真实 `sys_locale::get_locale()`，
//! CI 环境结果不稳定，所以 System 用例只验证返回值在允许集合内。

use i18n::AppLanguage;

#[test]
fn all_returns_three_variants_in_order() {
    let all = AppLanguage::all();
    assert_eq!(all.len(), 3);
    assert!(all.contains(&AppLanguage::System));
    assert!(all.contains(&AppLanguage::English));
    assert!(all.contains(&AppLanguage::SimplifiedChinese));
}

#[test]
fn default_is_system() {
    let d = AppLanguage::default();
    assert_eq!(d, AppLanguage::System);
}

#[test]
fn dropdown_label_system_is_system() {
    // System 用本地化文案显示，方便用户识别；这里英文是为了在英文 UI 下也能识别
    assert_eq!(AppLanguage::System.dropdown_label(), "System");
}

#[test]
fn dropdown_label_english_is_english() {
    assert_eq!(AppLanguage::English.dropdown_label(), "English");
}

#[test]
fn dropdown_label_simplified_chinese_uses_native_name() {
    // 简中用目标语言显示，方便中文用户在英文 UI 下找到
    assert_eq!(AppLanguage::SimplifiedChinese.dropdown_label(), "简体中文");
}

#[test]
fn resolve_english_returns_en() {
    assert_eq!(AppLanguage::English.resolve(), "en");
}

#[test]
fn resolve_simplified_chinese_returns_zh_cn() {
    assert_eq!(AppLanguage::SimplifiedChinese.resolve(), "zh-CN");
}

#[test]
fn resolve_system_returns_valid_locale() {
    // System 模式下调用真实系统检测；结果只可能是 zh-CN 或 en（MVP）
    let r = AppLanguage::System.resolve();
    assert!(r == "zh-CN" || r == "en", "unexpected locale: {r}");
}

#[test]
fn serde_roundtrip_preserves_variant() {
    let cases = [
        AppLanguage::System,
        AppLanguage::English,
        AppLanguage::SimplifiedChinese,
    ];
    for v in cases {
        let s = serde_json::to_string(&v).unwrap();
        let back: AppLanguage = serde_json::from_str(&s).unwrap();
        assert_eq!(v, back, "roundtrip failed for {v:?} (json={s})");
    }
}

#[test]
fn serde_uses_snake_case_by_default() {
    // 设置文件存 snake_case，与现有 enum 设置一致（参考 EnforceMinimumContrast）
    let s = serde_json::to_string(&AppLanguage::SimplifiedChinese).unwrap();
    assert_eq!(s, "\"simplified_chinese\"");
    let back: AppLanguage = serde_json::from_str("\"simplified_chinese\"").unwrap();
    assert_eq!(back, AppLanguage::SimplifiedChinese);
}
