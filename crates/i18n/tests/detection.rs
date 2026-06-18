//! BCP-47 归一化逻辑测试。`detect_system_language()` 本身依赖 `sys_locale::get_locale()`，
//! 我们通过 `normalize()` 内部函数验证归一化逻辑。

use i18n::detection::normalize;

#[test]
fn normalize_posix_zh_cn() {
    assert_eq!(normalize("zh_CN.UTF-8"), Some("zh-CN"));
}

#[test]
fn normalize_posix_zh_cn_no_encoding() {
    assert_eq!(normalize("zh_CN"), Some("zh-CN"));
}

#[test]
fn normalize_lowercase_zh_cn() {
    assert_eq!(normalize("zh-cn"), Some("zh-CN"));
}

#[test]
fn normalize_macos_zh_hans_cn() {
    // macOS NSLocale 返回 BCP-47：zh-Hans-CN
    assert_eq!(normalize("zh-Hans-CN"), Some("zh-CN"));
}

#[test]
fn normalize_macos_zh_hant_tw_falls_back_to_zh_cn() {
    // MVP：繁中也归一到 zh-CN（简中资源覆盖）
    assert_eq!(normalize("zh-Hant-TW"), Some("zh-CN"));
}

#[test]
fn normalize_posix_zh_tw_falls_back_to_zh_cn() {
    assert_eq!(normalize("zh_TW.UTF-8"), Some("zh-CN"));
}

#[test]
fn normalize_english_us() {
    assert_eq!(normalize("en_US.UTF-8"), Some("en"));
}

#[test]
fn normalize_english_uk() {
    assert_eq!(normalize("en-GB"), Some("en"));
}

#[test]
fn normalize_japanese_falls_back_to_en() {
    // MVP 只有 zh-CN + en；其他语言 fallback 到 en
    assert_eq!(normalize("ja_JP"), Some("en"));
}

#[test]
fn normalize_empty_input() {
    assert_eq!(normalize(""), None);
}

#[test]
fn normalize_unrelated_input() {
    assert_eq!(normalize("C"), Some("en"));
    assert_eq!(normalize("POSIX"), Some("en"));
}

#[test]
fn normalize_trims_whitespace() {
    // sys-locale 偶尔会返回带空白的环境变量值；trim 保证健壮性
    assert_eq!(normalize(" zh-CN "), Some("zh-CN"));
    assert_eq!(normalize("\ten-US\n"), Some("en"));
}
