//! `i18n-check`：核对源码里的 `t!()` key 与 locale YAML 是否一致。
//!
//! 报告：
//! - **missing**：代码里有、但 YAML 里没有翻译的 key（会以退出码 1 失败）
//! - **orphan**：YAML 里有、但代码里已经没有引用的 key（仅提示）
//!
//! 用法：
//! ```text
//! i18n-check [--source PATH] [--locales PATH]
//! ```

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Result;
use clap::Parser;
use i18n_tools::{collect_t_keys, load_ignore_config, read_yaml_keys, walk_source_files};

#[derive(Parser)]
#[command(
    name = "i18n-check",
    about = "核对 t!() key 与 zh-CN locale YAML 是否一致"
)]
struct Cli {
    /// 要扫描的源码根目录。
    #[arg(long, default_value = ".")]
    source: PathBuf,

    /// locale 目录（其下所有 .yml 都会被读取）。
    #[arg(long, default_value = "crates/i18n/_locales/zh-CN/")]
    locales: PathBuf,
}

fn main() -> ExitCode {
    match run() {
        Ok(missing) => {
            if missing {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            }
        }
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<bool> {
    let cli = Cli::parse();
    let cfg = load_ignore_config(&std::env::current_dir()?)?;
    let files = walk_source_files(&cli.source, &cfg.exclude_paths)?;

    // 1) 收集代码里所有 t!() key。
    let mut code_keys: BTreeSet<String> = BTreeSet::new();
    for file in &files {
        let content = match fs::read_to_string(file) {
            Ok(c) => c,
            Err(_) => continue,
        };
        if cfg
            .exclude_file_contains
            .iter()
            .any(|marker| content.contains(marker.as_str()))
        {
            continue;
        }
        for k in collect_t_keys(&content) {
            code_keys.insert(k);
        }
    }

    // 2) 读取 locale 目录下所有 YAML。
    let mut yaml_keys: BTreeSet<String> = BTreeSet::new();
    for entry in fs::read_dir(&cli.locales)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("yml") {
            yaml_keys.extend(read_yaml_keys(&path)?);
        }
    }

    // 3) diff。
    let missing: Vec<&String> = code_keys.difference(&yaml_keys).collect();
    let orphan: Vec<&String> = yaml_keys.difference(&code_keys).collect();

    println!(
        "code keys: {}, locale keys: {}",
        code_keys.len(),
        yaml_keys.len()
    );

    if !missing.is_empty() {
        eprintln!("\nmissing translations ({}):", missing.len());
        for k in &missing {
            eprintln!("  - {k}");
        }
    } else {
        println!("no missing translations.");
    }

    if !orphan.is_empty() {
        eprintln!("\norphan keys (in YAML but not in code): {}", orphan.len());
        for k in &orphan {
            eprintln!("  - {k}");
        }
    }

    Ok(!missing.is_empty())
}
