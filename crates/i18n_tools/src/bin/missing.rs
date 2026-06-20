//! Dump missing t!() keys as a YAML file with empty zh-CN translations.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use i18n_tools::{collect_t_keys, load_ignore_config, read_yaml_keys, walk_source_files};

#[derive(Parser)]
#[command(name = "i18n-missing")]
struct Cli {
    #[arg(long, default_value = ".")]
    source: PathBuf,
    #[arg(long, default_value = "crates/i18n/_locales/zh-CN/")]
    locales: PathBuf,
    /// Write missing keys (with empty zh-CN) to this YAML file.
    #[arg(long, default_value = "/tmp/missing.yml")]
    out: PathBuf,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let cfg = load_ignore_config(&std::env::current_dir()?)?;
    let files = walk_source_files(&cli.source, &cfg.exclude_paths)?;

    let mut code_keys: BTreeSet<String> = BTreeSet::new();
    for file in &files {
        let content = match fs::read_to_string(file) {
            Ok(c) => c,
            Err(_) => continue,
        };
        if cfg.exclude_file_contains.iter().any(|m| content.contains(m.as_str())) {
            continue;
        }
        for k in collect_t_keys(&content) {
            code_keys.insert(k);
        }
    }

    let mut yaml_keys: BTreeSet<String> = BTreeSet::new();
    for entry in fs::read_dir(&cli.locales)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("yml") {
            yaml_keys.extend(read_yaml_keys(&path)?);
        }
    }

    let missing: BTreeSet<&String> = code_keys.difference(&yaml_keys).collect();
    println!("code: {}, yaml: {}, missing: {}", code_keys.len(), yaml_keys.len(), missing.len());

    // Build YAML using serde_yaml to ensure proper escaping of multi-line/special keys.
    use std::collections::BTreeMap;
    let mut map: BTreeMap<&str, BTreeMap<&str, String>> = BTreeMap::new();
    for k in &missing {
        let mut inner: BTreeMap<&str, String> = BTreeMap::new();
        inner.insert("zh-CN", String::new());
        map.insert(k.as_str(), inner);
    }

    let serialized = serde_yaml::to_string(&map)?;
    let out = format!("_version: 2\n\n{serialized}");
    fs::write(&cli.out, out)?;
    println!("Wrote missing YAML to {}", cli.out.display());
    Ok(())
}
