//! Warp 国际化（i18n）提取与检查的核心引擎。
//!
//! 本 crate 提供给 `i18n-extract` / `i18n-check` 两个二进制共享的逻辑：
//! - [`apply_patterns`]：把硬编码的 UI 字符串字面量改写为 `i18n::t!(...)` 调用
//! - [`walk_source_files`]：按 `.i18n-ignore.toml` 的排除规则枚举 Rust 源文件
//! - [`collect_t_keys`]：收集文件中已有的 `t!()` key（供 check 使用）
//! - YAML 读写（[`read_yaml_keys`] / [`write_yaml_keys`]）处理 v2 格式的 locale 文件
//!
//! v2 YAML 格式（见 `crates/i18n/_locales/zh-CN/*.yml`）：
//!
//! ```yaml
//! _version: 2
//!
//! "Cancel":
//!   zh-CN: 取消
//! ```

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::{Context, Result};
use globset::{Glob, GlobSet, GlobSetBuilder};
use regex::{Captures, Regex};

/// 匹配一个 Rust 字符串字面量的“内容”（引号之间的字节），允许 `\"`、`\\` 等转义。
/// 捕获组 1 = 原始字面量内容（转义尚未解码）。
const STRING_LITERAL: &str = r#"((?:[^"\\]|\\.)*)"#;

/// 一条改写规则：用 `regex` 定位 UI 字符串位置，用 `replacement` 模板（含 `$1`/`$2`）
/// 重建为 `i18n::t!` 调用，`key_group` 指出哪个捕获组保存了字符串字面量内容。
struct Pattern {
    regex: Regex,
    replacement: String,
    key_group: usize,
}

/// 返回所有内置的 UI 字符串改写规则（编译一次后复用）。
fn patterns() -> &'static [Pattern] {
    static PATTERNS: OnceLock<Vec<Pattern>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        let lit = STRING_LITERAL;
        // 展开模式：每个规则把 `"X"` 包成 `i18n::t!("X")`，并在需要时补 `.to_string()`。
        let mk = |regex_src: &str, replacement: &str, key_group: usize| Pattern {
            regex: Regex::new(regex_src).unwrap(),
            replacement: replacement.to_string(),
            key_group,
        };
        vec![
            // 1. `.label("X".to_string())` → `.label(i18n::t!("X").to_string())`
            mk(
                &format!(r#"\.label\("{lit}"\.to_string\(\)\)"#),
                r#".label(i18n::t!("$1").to_string())"#,
                1,
            ),
            // 2. `.with_text_label("X".to_string())` → `.with_text_label(i18n::t!("X").to_string())`
            mk(
                &format!(r#"\.with_text_label\("{lit}"\.to_string\(\)\)"#),
                r#".with_text_label(i18n::t!("$1").to_string())"#,
                1,
            ),
            // 3. `Category::new("X",` → `Category::new(i18n::t!("X").to_string(),`
            //    （`Category::new` 接收 `impl Into<String>`，所以一律补 `.to_string()`，
            //     同时兼容原本是 `&'static str` 的情况）
            mk(
                &format!(r#"Category::new\("{lit}", "#),
                r#"Category::new(i18n::t!("$1").to_string(), "#,
                1,
            ),
            // 4. `render_body_item_label::<T>("X".to_string(),` → 包成 t!
            //    turbofish 中的类型原样保留（捕获组 1），字面量内容是捕获组 2。
            mk(
                &format!(
                    r#"render_body_item_label::<([A-Za-z_][A-Za-z0-9_]*)>\("{lit}"\.to_string\(\),"#,
                ),
                r#"render_body_item_label::<$1>(i18n::t!("$2").to_string(),"#,
                2,
            ),
            // 5. `.span("X".to_string())` → `.span(i18n::t!("X").to_string())`
            mk(
                &format!(r#"\.span\("{lit}"\.to_string\(\)\)"#),
                r#".span(i18n::t!("$1").to_string())"#,
                1,
            ),
            // 5b. `.span("X")`（裸 `&str`/`Cow` 形参，UI builder 的 `span`）
            //     → `.span(i18n::t!("X"))`（形参为 `impl Into<Cow<'static, str>>`）
            //     末尾要求 `")`，故不会误匹配上方带 `.to_string()` 的形式。
            mk(
                &format!(r#"\.span\("{lit}"\)"#),
                r#".span(i18n::t!("$1"))"#,
                1,
            ),
            // 6. `ui_builder().label("X")`（无 `.to_string`，形参为 `&str`）
            //    → `ui_builder().label(&i18n::t!("X"))`
            mk(
                &format!(r#"ui_builder\(\)\.label\("{lit}"\)"#),
                r#"ui_builder().label(&i18n::t!("$1"))"#,
                1,
            ),
            // 7. `.tool_tip("X".to_string())` → `.tool_tip(i18n::t!("X").to_string())`
            mk(
                &format!(r#"\.tool_tip\("{lit}"\.to_string\(\)\)"#),
                r#".tool_tip(i18n::t!("$1").to_string())"#,
                1,
            ),
            // 8. `.with_centered_text_label("X".to_string())`
            //    → `.with_centered_text_label(i18n::t!("X").to_string())`
            mk(
                &format!(r#"\.with_centered_text_label\("{lit}"\.to_string\(\)\)"#),
                r#".with_centered_text_label(i18n::t!("$1").to_string())"#,
                1,
            ),
            // 9. `.paragraph("X".to_string())` → `.paragraph(i18n::t!("X").to_string())`
            mk(
                &format!(r#"\.paragraph\("{lit}"\.to_string\(\)\)"#),
                r#".paragraph(i18n::t!("$1").to_string())"#,
                1,
            ),
            // 10. `.paragraph("X")`（裸 `&str`/`Cow` 形参）
            //     → `.paragraph(i18n::t!("X"))`（形参为 `impl Into<Cow<'static, str>>`）
            //     末尾要求 `")`，故不会误匹配上方带 `.to_string()` 的形式。
            mk(
                &format!(r#"\.paragraph\("{lit}"\)"#),
                r#".paragraph(i18n::t!("$1"))"#,
                1,
            ),
            // 11. `.with_description("X".to_string())` → `.with_description(i18n::t!("X").to_string())`
            mk(
                &format!(r#"\.with_description\("{lit}"\.to_string\(\)\)"#),
                r#".with_description(i18n::t!("$1").to_string())"#,
                1,
            ),
            // 12. `.set_fallback_display_title("X".to_string())`
            //     → `.set_fallback_display_title(i18n::t!("X").to_string())`
            mk(
                &format!(r#"\.set_fallback_display_title\("{lit}"\.to_string\(\)\)"#),
                r#".set_fallback_display_title(i18n::t!("$1").to_string())"#,
                1,
            ),
            // 13. `.with_badge("X".to_string())` → `.with_badge(i18n::t!("X").to_string())`
            mk(
                &format!(r#"\.with_badge\("{lit}"\.to_string\(\)\)"#),
                r#".with_badge(i18n::t!("$1").to_string())"#,
                1,
            ),
            // 14. `set_placeholder_text("X"` → `set_placeholder_text(i18n::t!("X").to_string()`
            //     （形参为 `impl Into<String>`，按既有约定补 `.to_string()`；
            //      不吞逗号——由原文保留。仅匹配紧跟 `("` 的字面量，已 `t!` 化的调用天然不匹配。）
            mk(
                &format!(r#"set_placeholder_text\("{lit}""#),
                r#"set_placeholder_text(i18n::t!("$1").to_string()"#,
                1,
            ),
            // 15. `HeaderContent::simple("X")` → `HeaderContent::simple(i18n::t!("X").to_string())`
            //     （形参为 `impl Into<String>`）
            mk(
                &format!(r#"HeaderContent::simple\("{lit}"\)"#),
                r#"HeaderContent::simple(i18n::t!("$1").to_string())"#,
                1,
            ),
            // 16. `Text::new("X")` → `Text::new(i18n::t!("X"))`
            //     （形参为 `impl Into<Cow<'static, str>>`；前置 `\b` 防止匹配 `FooText::new`）
            mk(
                &format!(r#"\bText::new\("{lit}"\)"#),
                r#"Text::new(i18n::t!("$1"))"#,
                1,
            ),
            // 17. `.with_tooltip("X")` → `.with_tooltip(i18n::t!("X").to_string())`
            //     （形参为 `impl Into<String>`；调用点一律是裸 `&str`，无 `.to_string()` 形式）
            mk(
                &format!(r#"\.with_tooltip\("{lit}"\)"#),
                r#".with_tooltip(i18n::t!("$1").to_string())"#,
                1,
            ),
            // 18. `write!(f, "X")`（Display impl 中的简单字面量）
            //     → `write!(f, "{}", i18n::t!("X"))`
            //     约束：字面量首字符大写、长度 3–50、不含 `{}`/`"`/`\\`。
            //     - `\b` 前置避免匹配 `rewrite!` 这类自定义宏；
            //     - 首字符 `[A-Z]` 排除小写/符号开头，也排除已改写的 `write!(f, "{}", ...)`
            //       （`{` 不匹配 `[A-Z]`），保证幂等；
            //     - `{2,49}` 复述：初始 1 + 2..49 = 总长 3..50（解码后）；
            //     - `i18n::t!` 返回 `Cow<str>`，实现 `Display`，故 `{}` 格式化合法。
            mk(
                r#"\bwrite!\(f,\s*"([A-Z](?:[^"{}\\]|\\.){2,49})"\)"#,
                r#"write!(f, "{}", i18n::t!("$1"))"#,
                1,
            ),
            // 19. `Some("X".into())` → `Some(i18n::t!("X").into())`
            //     设置项描述：`render_dropdown_item` / `render_body_item` 等的
            //     `Option<String>`（或 `Option<Cow>`）描述字段。约束：首字符 `[A-Z]`、
            //     长度 6–151（`[A-Z]` + 5..150），排除短按钮文案与小写/符号开头的非 UI
            //     字符串。已 `t!` 化的调用天然不匹配（`Some(` 后紧跟 `i18n::t!`，而非 `"`）。
            mk(
                r#"Some\("([A-Z][^"]{5,150})"\.into\(\)\)"#,
                r#"Some(i18n::t!("$1").into())"#,
                1,
            ),
            // 20. `Some("X".to_string())` → `Some(i18n::t!("X").to_string())`
            //     同上，针对 `.to_string()` 形式（`Option<String>` 描述字段）。
            mk(
                r#"Some\("([A-Z][^"]{5,150})"\.to_string\(\)\)"#,
                r#"Some(i18n::t!("$1").to_string())"#,
                1,
            ),
        ]
    })
}

/// 解码 Rust 字符串字面量的原始内容（去掉转义的反斜杠）。
///
/// 处理常见转义：`\"` `\\` `\n` `\t` `\r` `\0` `\'`。其它（如 `\u{...}`）保持原样，
/// 因为 UI 字符串几乎总是直接写 Unicode 字符而不用 `\u{}`。多字节 UTF-8 字符按 `char`
/// 迭代，不会被破坏。
fn unescape_rust_string(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('r') => out.push('\r'),
                Some('0') => out.push('\0'),
                Some('\'') => out.push('\''),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// 行级排除：日志相关的调用/宏，这些字符串不属于用户可见 UI。
fn is_log_line(line: &str) -> bool {
    const MARKERS: &[&str] = &["log::", "debug!", "warn!", "error!", "info!", "trace!"];
    MARKERS.iter().any(|m| line.contains(m))
}

/// 把 `.i18n-ignore.toml` 中的字符串排除正则编译好，供 [`apply_patterns_with`] 使用。
#[derive(Default)]
pub struct CompiledStringExcludes {
    regexes: Vec<Regex>,
}

impl CompiledStringExcludes {
    pub fn from_patterns(patterns: &[String]) -> Self {
        let regexes = patterns.iter().filter_map(|p| Regex::new(p).ok()).collect();
        CompiledStringExcludes { regexes }
    }

    fn is_excluded(&self, key: &str) -> bool {
        self.regexes.iter().any(|r| r.is_match(key))
    }
}

/// 对单个 key 应用排除规则：返回 true 表示该字符串不应被提取/改写。
fn should_exclude_key(key: &str, excludes: &CompiledStringExcludes) -> bool {
    excludes.is_excluded(key)
}

/// 把文件内容里的 UI 字符串模式改写为 `i18n::t!(...)`，并返回所有提取出的 key。
///
/// 幂等：已经被 `t!()` 包裹的字符串不会被二次包裹（模式要求方法名后紧跟 `"`，
/// 而已包裹的内容是 `i18n::t!("...`，自然不会匹配）。
///
/// 排除：跳过日志宏行、`#[cfg(test)]` 块内的内容，以及（通过 `excludes`）URL、
/// 文件路径、serde key 等明显不是 UI 的字符串。
pub fn apply_patterns(content: &str) -> (String, Vec<String>) {
    apply_patterns_with(content, &CompiledStringExcludes::default())
}

/// [`apply_patterns`] 的可配置版本：额外根据 `excludes` 跳过非 UI 字符串。
pub fn apply_patterns_with(
    content: &str,
    excludes: &CompiledStringExcludes,
) -> (String, Vec<String>) {
    let mut keys = Vec::new();
    let mut out = String::with_capacity(content.len());

    // `#[cfg(test)]` 块跟踪：pending = 看到了属性、等待开括号；
    // depth>0 = 处于块内，按花括号深度跳过直到块结束。
    let mut pending_cfg_test = false;
    let mut test_block_depth: i32 = 0;

    for line in content.lines() {
        let in_test_block = test_block_depth > 0;

        if !in_test_block && line.contains("#[cfg(test)]") {
            pending_cfg_test = true;
        }

        if pending_cfg_test || in_test_block {
            // 累计花括号深度，找到块的开/闭。
            let delta = net_braces(line);
            if pending_cfg_test {
                if delta > 0 {
                    // 开括号与本行出现：进入块。
                    pending_cfg_test = false;
                    test_block_depth = delta;
                }
                // delta<=0：属性后还没遇到 `{`，继续等待。
            } else if in_test_block {
                test_block_depth += delta;
            }
            out.push_str(line);
            out.push('\n');
            continue;
        }

        if is_log_line(line) {
            out.push_str(line);
            out.push('\n');
            continue;
        }

        let mut new_line = String::from(line);
        for pattern in patterns() {
            new_line = pattern
                .regex
                .replace_all(&new_line, |caps: &Captures| {
                    let raw = &caps[pattern.key_group];
                    let key = unescape_rust_string(raw);
                    // 空字符串、纯空白不是 UI 文案；config 排除规则也一并检查。
                    if key.trim().is_empty() || should_exclude_key(&key, excludes) {
                        // 保留原文，不改写、不记录 key。
                        return caps[0].to_string();
                    }
                    let mut rep = String::new();
                    caps.expand(&pattern.replacement, &mut rep);
                    keys.push(key);
                    rep
                })
                .into_owned();
        }
        out.push_str(&new_line);
        out.push('\n');
    }

    // 去掉因 lines() 末尾处理可能多加的最后换行：若原文不以换行结尾则对齐。
    if !content.ends_with('\n') && out.ends_with('\n') {
        out.pop();
    }

    (out, keys)
}

/// 统计一行内的净花括号数量（`{` 计 +1，`}` 计 -1），跳过字符串字面量与行注释，
/// 用于跟踪 `#[cfg(test)]` 块的边界。对块注释与字符字面量内的花括号不做处理
/// （在测试块中极少出现）。
fn net_braces(line: &str) -> i32 {
    let bytes = line.as_bytes();
    let mut i = 0;
    let mut delta = 0i32;
    let mut in_str = false;
    while i < bytes.len() {
        let c = bytes[i];
        if in_str {
            if c == b'\\' {
                i += 2;
                continue;
            }
            if c == b'"' {
                in_str = false;
            }
            i += 1;
            continue;
        }
        if c == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            break;
        }
        match c {
            b'"' => in_str = true,
            b'{' => delta += 1,
            b'}' => delta -= 1,
            _ => {}
        }
        i += 1;
    }
    delta
}

/// 收集文件内容里所有 `t!("key")` / `i18n::t!("key")` 调用的 key（解码转义后）。
pub fn collect_t_keys(content: &str) -> Vec<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r#"(?:i18n::)?t!\("((?:[^"\\]|\\.)*)"\)"#).unwrap());
    re.captures_iter(content)
        .map(|c| unescape_rust_string(&c[1]))
        .collect()
}

/// 枚举 `root` 下的 `.rs` 文件，跳过匹配 `excludes`（支持 `**` glob）的路径。
pub fn walk_source_files(root: &Path, excludes: &[String]) -> Result<Vec<PathBuf>> {
    let globset = build_globset(excludes)?;
    let mut files = Vec::new();
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| {
            // 提前剪枝被排除的目录，避免走进 target/ 等。
            if e.file_type().is_dir() {
                return !globset.is_match(e.path());
            }
            true
        })
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        if globset.is_match(path) {
            continue;
        }
        files.push(path.to_path_buf());
    }
    Ok(files)
}

/// 把 `.i18n-ignore.toml` 中的 path glob 编译成 `GlobSet`。
///
/// 形如 `target/`、`crates/warp_completer/`（不含 `*`）的目录式模式会被规范化为
/// `**/target/**`，使其能匹配路径任意位置的该目录。
fn build_globset(excludes: &[String]) -> Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for raw in excludes {
        let trimmed = raw.trim_end_matches('/');
        let glob = if trimmed.contains('*') {
            trimmed.to_string()
        } else {
            format!("**/{trimmed}/**")
        };
        builder.add(Glob::new(&glob)?);
    }
    Ok(builder.build()?)
}

/// `.i18n-ignore.toml` 的解析结果。
#[derive(Default, Debug)]
pub struct IgnoreConfig {
    pub exclude_paths: Vec<String>,
    pub exclude_file_contains: Vec<String>,
    pub exclude_strings: Vec<String>,
}

/// 读取并解析 `.i18n-ignore.toml`。文件不存在时返回空配置。
pub fn load_ignore_config(start_dir: &Path) -> Result<IgnoreConfig> {
    let path = start_dir.join(".i18n-ignore.toml");
    match fs::read_to_string(&path) {
        Ok(text) => Ok(parse_ignore_config(&text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(IgnoreConfig::default()),
        Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
    }
}

/// 极简的 flat-TOML 解析器：只识别
/// `key = [ "a", "b", ... ]` 形式的字符串数组（支持多行）。
pub fn parse_ignore_config(text: &str) -> IgnoreConfig {
    let mut cfg = IgnoreConfig::default();
    let mut current: Option<String> = None; // 正在收集的数组 key

    let push_to = |cfg: &mut IgnoreConfig, key: &str, val: String| match key {
        "exclude_paths" => cfg.exclude_paths.push(val),
        "exclude_file_contains" => cfg.exclude_file_contains.push(val),
        "exclude_strings" => cfg.exclude_strings.push(val),
        _ => {}
    };

    for line in text.lines() {
        let t = line.trim();
        if current.is_none() {
            if let Some(eq) = t.find('=') {
                let key = t[..eq].trim().to_string();
                let rest = t[eq + 1..].trim();
                if rest.starts_with('[') {
                    current = Some(key.clone());
                    for v in extract_string_literals(rest) {
                        push_to(&mut cfg, &key, v);
                    }
                    if rest.contains(']') {
                        current = None;
                    }
                }
            }
        } else {
            let key = current.as_ref().unwrap().clone();
            for v in extract_string_literals(t) {
                push_to(&mut cfg, &key, v);
            }
            if t.contains(']') {
                current = None;
            }
        }
    }
    cfg
}

/// 从一行文本里提取所有 `"..."` 字符串字面量的内容（支持 `\"` 转义）。
fn extract_string_literals(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            let mut j = i + 1;
            while j < bytes.len() {
                if bytes[j] == b'\\' {
                    j += 2;
                    continue;
                }
                if bytes[j] == b'"' {
                    break;
                }
                j += 1;
            }
            if j < bytes.len() {
                if let Ok(s) = std::str::from_utf8(&bytes[i + 1..j]) {
                    out.push(unescape_rust_string(s));
                }
                i = j + 1;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// 读取 v2 格式的 YAML，返回其中所有顶层 key（不含 `_version`）。
pub fn read_yaml_keys(path: &Path) -> Result<BTreeSet<String>> {
    let text =
        fs::read_to_string(path).with_context(|| format!("reading yaml {}", path.display()))?;
    let map: serde_yaml::Mapping = serde_yaml::from_str(&text).unwrap_or_default();
    Ok(map
        .into_iter()
        .filter_map(|(k, _)| k.as_str().map(|s| s.to_string()))
        .filter(|k| k != "_version")
        .collect())
}

/// 把一组 key 以 v2 格式写入 YAML（翻译值留空，由译者填充）。
pub fn write_yaml_keys(keys: &BTreeSet<String>, path: &Path) -> Result<()> {
    let mut out = String::from("_version: 2\n\n");
    for key in keys {
        out.push_str(&format!("{}:\n  zh-CN: \"\"\n", yaml_scalar(key)));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    fs::write(path, out).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

/// 把 key 输出为 YAML 双引号标量，与现有 locale 文件（`"Cancel":`）的格式保持一致，
/// 同时保证含空格/冒号等特殊字符的 key 始终合法。非 ASCII（如中文）原样保留。
fn yaml_scalar(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
