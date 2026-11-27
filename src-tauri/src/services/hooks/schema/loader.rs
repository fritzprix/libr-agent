use super::parser::{parse_hooks_file, parse_hooks_inline};
use super::HooksConfig;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub fn load_hooks_from_root(root: &Path) -> Result<HooksConfig, String> {
    let hooks_dir = root.join("hooks");
    let claude_path = hooks_dir.join("hooks.json");
    let cursor_path = hooks_dir.join("hooks-cursor.json");

    let mut merged = HooksConfig {
        version: None,
        events: BTreeMap::new(),
        unsupported_events: BTreeMap::new(),
        unsupported_handlers: BTreeMap::new(),
    };
    let mut found_any = false;

    if claude_path.is_file() {
        let piece = parse_hooks_file(&claude_path)?;
        merge_hooks_config(&mut merged, piece)?;
        found_any = true;
    }
    if cursor_path.is_file() {
        let piece = parse_hooks_file(&cursor_path)?;
        merge_hooks_config(&mut merged, piece)?;
        found_any = true;
    }

    if !found_any {
        return Err(format!(
            "no hook config found under {}",
            hooks_dir.display()
        ));
    }

    Ok(merged)
}

/// Load and merge explicit hook config files — the custom `hooks` paths a
/// `.claude-plugin/plugin.json` manifest declares (#2921). The files
/// replace the default `hooks/hooks.json` discovery, so at least one must
/// exist (callers validate existence before calling).
pub fn load_hooks_from_config_files(files: &[PathBuf]) -> Result<HooksConfig, String> {
    if files.is_empty() {
        return Err("no hook config files given".to_string());
    }
    let mut merged = HooksConfig {
        version: None,
        events: BTreeMap::new(),
        unsupported_events: BTreeMap::new(),
        unsupported_handlers: BTreeMap::new(),
    };
    for file in files {
        let piece = parse_hooks_file(file)?;
        merge_hooks_config(&mut merged, piece)?;
    }
    Ok(merged)
}

fn merge_hooks_config(dst: &mut HooksConfig, src: HooksConfig) -> Result<(), String> {
    if dst.version.is_none() {
        dst.version = src.version;
    }
    for (event, groups) in src.events {
        dst.events.entry(event).or_default().extend(groups);
    }
    for (raw_event, entries) in src.unsupported_events {
        dst.unsupported_events
            .entry(raw_event)
            .or_default()
            .extend(entries);
    }
    for (event, entries) in src.unsupported_handlers {
        dst.unsupported_handlers
            .entry(event)
            .or_default()
            .extend(entries);
    }
    Ok(())
}

/// Parse the inline hooks objects a manifest declares and merge them into
/// `dst` (#3005). Every object has the same shape as a hook config file's
/// `"hooks"` member.
pub fn merge_hooks_inline(
    dst: &mut HooksConfig,
    values: &[Value],
    label: &str,
) -> Result<(), String> {
    for value in values {
        let parsed = parse_hooks_inline(value, label)?;
        merge_hooks_config(dst, parsed)?;
    }
    Ok(())
}
