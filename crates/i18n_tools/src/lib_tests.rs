use std::collections::BTreeSet;

use super::*;

// ---------- 各模式：apply → 验证改写 + 提取出的 key ----------

#[test]
fn rewrites_label_with_to_string() {
    let src = r#"    let b = foo.label("Cancel".to_string());
"#;
    let (out, keys) = apply_patterns(src);
    assert_eq!(
        out,
        r#"    let b = foo.label(i18n::t!("Cancel").to_string());
"#
    );
    assert_eq!(keys, vec!["Cancel".to_string()]);
}

#[test]
fn rewrites_with_text_label_with_to_string() {
    let src = r#"    ui.with_text_label("Email".to_string())
"#;
    let (out, keys) = apply_patterns(src);
    assert_eq!(
        out,
        r#"    ui.with_text_label(i18n::t!("Email").to_string())
"#
    );
    assert_eq!(keys, vec!["Email".to_string()]);
}

#[test]
fn rewrites_category_new_first_arg() {
    let src = r#"    let c = Category::new("Themes", icon);
"#;
    let (out, keys) = apply_patterns(src);
    assert_eq!(
        out,
        r#"    let c = Category::new(i18n::t!("Themes").to_string(), icon);
"#
    );
    assert_eq!(keys, vec!["Themes".to_string()]);
}

#[test]
fn rewrites_render_body_item_label_turbofish() {
    let src = r#"    render_body_item_label::<Toggle>("Notifications".to_string(), model);
"#;
    let (out, keys) = apply_patterns(src);
    assert_eq!(
        out,
        r#"    render_body_item_label::<Toggle>(i18n::t!("Notifications").to_string(), model);
"#
    );
    assert_eq!(keys, vec!["Notifications".to_string()]);
}

#[test]
fn rewrites_span_with_to_string() {
    let src = r#"    let s = b.span("Hello".to_string());
"#;
    let (out, keys) = apply_patterns(src);
    assert_eq!(
        out,
        r#"    let s = b.span(i18n::t!("Hello").to_string());
"#
    );
    assert_eq!(keys, vec!["Hello".to_string()]);
}

#[test]
fn rewrites_ui_builder_label_str_arg() {
    let src = r#"    ui_builder().label("Account")
"#;
    let (out, keys) = apply_patterns(src);
    assert_eq!(
        out,
        r#"    ui_builder().label(&i18n::t!("Account"))
"#
    );
    assert_eq!(keys, vec!["Account".to_string()]);
}

#[test]
fn ui_builder_label_distinct_from_plain_label() {
    // .label("X")（无 to_string、且无 ui_builder() 前缀）不应被改写（保守起见）。
    let src = r#"    foo.label("Skip me")
"#;
    let (out, keys) = apply_patterns(src);
    assert_eq!(out, src);
    assert!(keys.is_empty());
}

#[test]
fn skips_empty_and_whitespace_only_strings() {
    let src = r#"    a.label("".to_string());
    b.label("   ".to_string());
    c.label("Real".to_string());
"#;
    let (out, keys) = apply_patterns(src);
    assert!(out.contains(r#".label(""#.to_string().as_str()) || out.contains(r#""""#));
    assert!(out.contains(r#""   ""#));
    assert!(out.contains(r#"i18n::t!("Real")"#));
    assert_eq!(keys, vec!["Real".to_string()]);
}

// ---------- 幂等性 ----------

#[test]
fn apply_patterns_is_idempotent() {
    let src = r#"    foo.label("Save".to_string()).span("Reset".to_string());
"#;
    let (out1, keys1) = apply_patterns(src);
    let (out2, keys2) = apply_patterns(&out1);
    assert_eq!(out1, out2, "二次 apply 不应再次改写");
    assert_eq!(keys1, vec!["Save".to_string(), "Reset".to_string()]);
    assert!(keys2.is_empty(), "二次 apply 不应再提取 key");
}

#[test]
fn does_not_double_wrap_already_wrapped() {
    let src = r#"    foo.label(i18n::t!("Save").to_string());
"#;
    let (out, keys) = apply_patterns(src);
    assert_eq!(out, src);
    assert!(keys.is_empty());
}

// ---------- 同行多个调用 ----------

#[test]
fn handles_multiple_matches_on_one_line() {
    let src = r#"    a.label("Yes".to_string()); b.label("No".to_string());
"#;
    let (out, keys) = apply_patterns(src);
    assert!(out.contains(r#"i18n::t!("Yes")"#));
    assert!(out.contains(r#"i18n::t!("No")"#));
    assert_eq!(keys, vec!["Yes".to_string(), "No".to_string()]);
}

// ---------- 排除：日志行、cfg(test) 块 ----------

#[test]
fn skips_log_macro_lines() {
    let src = r#"    warn!("something failed: {}", err);
    foo.label("Keep".to_string());
"#;
    let (out, keys) = apply_patterns(src);
    assert!(out.contains(r#"warn!"#));
    assert!(out.contains(r#"i18n::t!("Keep")"#));
    assert_eq!(keys, vec!["Keep".to_string()]);
}

#[test]
fn skips_cfg_test_block() {
    let src = "\
#[cfg(test)]
mod tests {
    fn helper() {
        let b = foo.label(\"Should Not Extract\".to_string());
    }

    #[test]
    fn it_works() {
        let b = foo.label(\"Still Inside\".to_string());
    }
}

fn real() {
    let b = foo.label(\"Real UI\".to_string());
}
";
    let (out, keys) = apply_patterns(src);
    assert!(
        out.contains(r#""Should Not Extract""#),
        "cfg(test) 内的字面量不应被改写"
    );
    assert!(out.contains(r#""Still Inside""#));
    assert!(out.contains(r#"i18n::t!("Real UI")"#));
    assert_eq!(keys, vec!["Real UI".to_string()]);
}

#[test]
fn skips_cfg_test_block_on_same_line_as_brace() {
    let src = "\
#[cfg(test)] mod tests {
    let b = foo.label(\"Inside\".to_string());
}

let b = foo.label(\"Outside\".to_string());
";
    let (out, keys) = apply_patterns(src);
    assert!(out.contains(r#""Inside""#));
    assert!(out.contains(r#"i18n::t!("Outside")"#));
    assert_eq!(keys, vec!["Outside".to_string()]);
}

// ---------- 字符串排除规则（exclude_strings） ----------

#[test]
fn exclude_strings_skips_urls_and_paths() {
    let excludes = CompiledStringExcludes::from_patterns(&[
        r"^https?://".to_string(),
        r"^/".to_string(),
        r"^[a-z_]+$".to_string(),
    ]);
    let src = r#"
    a.label("https://warp.dev".to_string());
    b.label("/etc/hosts".to_string());
    c.label("snake_case_key".to_string());
    d.label("Real Label".to_string());
"#;
    let (out, keys) = apply_patterns_with(src, &excludes);
    // 前三个保留原文、不提取；最后一个正常改写。
    assert!(out.contains(r#""https://warp.dev""#));
    assert!(out.contains(r#""/etc/hosts""#));
    assert!(out.contains(r#""snake_case_key""#));
    assert!(out.contains(r#"i18n::t!("Real Label")"#));
    assert_eq!(keys, vec!["Real Label".to_string()]);
}

// ---------- 转义字符 ----------

#[test]
fn handles_escaped_quotes_in_literal() {
    // 输入的源码行里，`"Say \"hi\""` 是带转义引号的字符串字面量（值为 Say "hi"）。
    let src = r#"    a.label("Say \"hi\"".to_string());
"#;
    let (out, keys) = apply_patterns(src);
    assert!(out.contains(r#"i18n::t!("Say \"hi\"")"#));
    assert_eq!(keys, vec![r#"Say "hi""#.to_string()]);
}

// ---------- collect_t_keys ----------

#[test]
fn collect_t_keys_finds_bare_and_qualified() {
    let src = r#"
    let a = t!("Cancel");
    let b = i18n::t!("Save");
    let c = i18n::t!("With \"quotes\"");
"#;
    let keys = collect_t_keys(src);
    assert_eq!(
        keys,
        vec![
            "Cancel".to_string(),
            "Save".to_string(),
            r#"With "quotes""#.to_string(),
        ]
    );
}

// ---------- unescape ----------

#[test]
fn unescape_handles_common_escapes() {
    assert_eq!(unescape_rust_string(r#"plain"#), "plain");
    assert_eq!(unescape_rust_string(r#"a\"b"#), "a\"b");
    assert_eq!(unescape_rust_string(r#"line\nbreak"#), "line\nbreak");
    assert_eq!(unescape_rust_string(r#"tab\tchar"#), "tab\tchar");
    assert_eq!(unescape_rust_string(r#"back\\slash"#), "back\\slash");
    // 非 ASCII 字符串不应被破坏。
    assert_eq!(unescape_rust_string("中文"), "中文");
}

// ---------- net_braces ----------

#[test]
fn net_braces_counts_correctly() {
    assert_eq!(net_braces("mod tests {"), 1);
    assert_eq!(net_braces("}"), -1);
    assert_eq!(net_braces("} else {"), 0);
    assert_eq!(net_braces(r#""{not counted}""#), 0);
    assert_eq!(net_braces("// {comment"), 0);
}

// ---------- parse_ignore_config ----------

#[test]
fn parses_ignore_config_multiline_arrays() {
    // 用 r##"..."## 以便内容里的 "#（来自 "#![cfg(test)]"）不会提前闭合 raw string。
    let text = r##"
# Paths to exclude
exclude_paths = [
  "target/",
  "**/tests/**",
  "crates/warp_completer/",
]
exclude_file_contains = ["#![cfg(test)]"]
exclude_strings = ["^https?://", "^/"]
"##;
    let cfg = parse_ignore_config(text);
    assert_eq!(
        cfg.exclude_paths,
        vec![
            "target/".to_string(),
            "**/tests/**".to_string(),
            "crates/warp_completer/".to_string(),
        ]
    );
    assert_eq!(cfg.exclude_file_contains, vec!["#![cfg(test)]".to_string()]);
    assert_eq!(
        cfg.exclude_strings,
        vec!["^https?://".to_string(), "^/".to_string()]
    );
}

#[test]
fn load_ignore_config_missing_file_is_empty() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = load_ignore_config(dir.path()).unwrap();
    assert!(cfg.exclude_paths.is_empty());
}

// ---------- build_globset / walk_source_files ----------

#[test]
fn walk_source_files_honors_excludes() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    // 建一些 .rs 文件
    std::fs::write(root.join("keep.rs"), "fn main() {}\n").unwrap();
    std::fs::create_dir_all(root.join("tests")).unwrap();
    std::fs::write(root.join("tests").join("skip.rs"), "fn t() {}\n").unwrap();
    std::fs::create_dir_all(root.join("target")).unwrap();
    std::fs::write(root.join("target").join("built.rs"), "fn b() {}\n").unwrap();
    // 非 .rs 文件应被忽略
    std::fs::write(root.join("readme.md"), "# hi").unwrap();

    let excludes = vec!["target/".to_string(), "**/tests/**".to_string()];
    let mut files: Vec<String> = walk_source_files(root, &excludes)
        .unwrap()
        .into_iter()
        .filter_map(|p| p.strip_prefix(root).ok()?.to_str().map(String::from))
        .collect();
    files.sort();
    assert_eq!(files, vec!["keep.rs".to_string()]);
}

// ---------- YAML 读写 round-trip ----------

#[test]
fn yaml_read_extracts_keys_without_version() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("x.yml");
    std::fs::write(
        &path,
        "_version: 2\n\n\"Cancel\":\n  zh-CN: 取消\n\"Save\":\n  zh-CN: 保存\n",
    )
    .unwrap();
    let keys = read_yaml_keys(&path).unwrap();
    let expected: BTreeSet<String> = ["Cancel".to_string(), "Save".to_string()]
        .into_iter()
        .collect();
    assert_eq!(keys, expected);
}

#[test]
fn yaml_write_round_trips_keys() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("out.yml");
    let keys: BTreeSet<String> = ["Cancel".to_string(), "Save".to_string(), "A:B".to_string()]
        .into_iter()
        .collect();
    write_yaml_keys(&keys, &path).unwrap();
    // 写出的内容能被 read_yaml_keys 读回同样的 key 集合。
    let read_back = read_yaml_keys(&path).unwrap();
    assert_eq!(read_back, keys);
    // 校验格式：包含 _version 头与空翻译槽。
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.starts_with("_version: 2"));
    assert!(text.contains("\"Cancel\":\n  zh-CN: \"\""));
}

// ---------- apply_patterns 边界情况 ----------

#[test]
fn leaves_non_matching_content_untouched() {
    let src = "fn main() {\n    println!(\"hi\");\n}\n";
    let (out, keys) = apply_patterns(src);
    assert_eq!(out, src);
    assert!(keys.is_empty());
}

#[test]
fn preserves_trailing_newline_state() {
    // 不以换行结尾的输入，输出也不应以换行结尾。
    let src = "    a.label(\"X\".to_string());";
    let (out, _keys) = apply_patterns(src);
    assert!(!out.ends_with('\n'));
    assert!(out.contains(r#"i18n::t!("X")"#));
}
