//! Smoke test：验证 zh-CN YAML 能被 rust-i18n 加载。
//!
//! 此测试在 Task 6 会被更完整的 yml_load.rs 替换。

// rust-i18n 的 `t!` 宏展开为 `crate::_rust_i18n_t!(...)`（裸 `crate::`，见
// rust-i18n 3.x lib.rs:144），而 `_rust_i18n_t!` 是由 `i18n!()` 在「调用方 crate
// 根」生成的。集成测试（tests/*.rs）是独立 crate，不继承 `i18n` 库里 `i18n!()`
// 的产物，故必须在此再次调用 `rust_i18n::i18n!`，否则编译报 E0433
// `could not find _rust_i18n_t in the crate root`。Task 6 的 yml_load.rs 亦然。
rust_i18n::i18n!("_locales", fallback = "en");

#[test]
fn zh_cn_lookup_works() {
    // 用 `locale=` 显式传参，避免 `set_locale` 改全局 `CURRENT_LOCALE` 在并行
    // 测试间产生竞态（rust-i18n 3.x 的 locale 是进程级全局）。
    // 这些 key 来自 _shared.yml / settings.yml
    assert_eq!(i18n::t!("Cancel", locale = "zh-CN").to_string(), "取消");
    assert_eq!(i18n::t!("Settings", locale = "zh-CN").to_string(), "设置");
    assert_eq!(i18n::t!("Language", locale = "zh-CN").to_string(), "语言");
}

#[test]
fn en_falls_back_to_key() {
    // en.yml 只有 _placeholder，Cancel 查不到 → `_tr!` 返回 key 字面量
    assert_eq!(i18n::t!("Cancel", locale = "en").to_string(), "Cancel");
}
