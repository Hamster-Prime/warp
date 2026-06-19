//! `i18n-extract`：扫描 Rust 源文件，把 UI 字符串改写为 `i18n::t!(...)`，
//! 并把提取出的 key 写成 v2 YAML。
//!
//! 用法：
//! ```text
//! i18n-extract [--dry-run] [--source PATH] [--yaml-out PATH] [--merge-existing]
//! ```

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Result;
use clap::Parser;
use i18n_tools::{
    CompiledStringExcludes, apply_patterns_with, load_ignore_config, read_yaml_keys,
    walk_source_files, write_yaml_keys,
};

#[derive(Parser)]
#[command(
    name = "i18n-extract",
    about = "扫描 Rust 源码，把 UI 字符串改写为 i18n::t! 并输出 YAML key 清单"
)]
struct Cli {
    /// 只报告、不修改文件。
    #[arg(long)]
    dry_run: bool,

    /// 要扫描的源码根目录。
    #[arg(long, default_value = ".")]
    source: PathBuf,

    /// 输出 YAML 路径。
    #[arg(long, default_value = "crates/i18n/_locales/zh-CN/extracted.yml")]
    yaml_out: PathBuf,

    /// 与现有 YAML 合并，而不是覆盖。
    #[arg(long)]
    merge_existing: bool,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let cfg = load_ignore_config(&std::env::current_dir()?)?;
    let string_excludes = CompiledStringExcludes::from_patterns(&cfg.exclude_strings);
    let files = walk_source_files(&cli.source, &cfg.exclude_paths)?;

    let mut all_keys: BTreeSet<String> = BTreeSet::new();
    let mut changed_files = 0usize;

    for file in &files {
        let content = match fs::read_to_string(file) {
            Ok(c) => c,
            Err(_) => continue,
        };
        // 跳过整个文件（如 crate 级 #![cfg(test)] 的纯测试 crate）。
        if cfg
            .exclude_file_contains
            .iter()
            .any(|marker| content.contains(marker.as_str()))
        {
            continue;
        }

        let (new_content, keys) = apply_patterns_with(&content, &string_excludes);
        all_keys.extend(keys);

        if new_content != content {
            changed_files += 1;
            if !cli.dry_run {
                fs::write(file, new_content)?;
            }
        }
    }

    // 合并现有 YAML（取并集），否则直接用本次结果。
    let final_keys = if cli.merge_existing && cli.yaml_out.exists() {
        let mut existing = read_yaml_keys(&cli.yaml_out)?;
        existing.extend(all_keys);
        existing
    } else {
        all_keys
    };

    if !cli.dry_run {
        write_yaml_keys(&final_keys, &cli.yaml_out)?;
    }

    let mode = if cli.dry_run { "[dry-run] " } else { "" };
    println!(
        "{mode}scanned {} file(s), {} would change, {} unique key(s)",
        files.len(),
        changed_files,
        final_keys.len(),
    );

    if cli.dry_run && !final_keys.is_empty() {
        println!("\nextracted keys:");
        for k in &final_keys {
            println!("  - {k}");
        }
    }

    Ok(())
}
