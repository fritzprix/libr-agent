//! Frontmatter, path-safety and namespacing helpers (#3043).

use crate::services::hooks::registry::PluginSourceRecord;
use std::fs;
use std::path::{Path, PathBuf};

/// Minimal `---`-delimited frontmatter reader: `key: value` lines only,
/// quotes stripped. Anything before the closing fence is not part of the
/// body. A file without frontmatter returns an empty map and the full text.
pub(crate) fn split_frontmatter(raw: &str) -> (std::collections::BTreeMap<String, String>, String) {
    let normalized = raw.strip_prefix('\u{feff}').unwrap_or(raw);
    let mut map = std::collections::BTreeMap::new();
    let Some(rest) = normalized
        .strip_prefix("---\n")
        .or_else(|| normalized.strip_prefix("---\r\n"))
    else {
        return (map, normalized.to_string());
    };
    let mut offset = 0;
    for raw_line in rest.split_inclusive('\n') {
        offset += raw_line.len();
        let line = raw_line.trim_end_matches(['\n', '\r']);
        if line == "---" {
            let body = &rest[offset..];
            return (map, body.trim_start_matches(['\n', '\r']).to_string());
        }
        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim();
            if key.is_empty() {
                continue;
            }
            let value = value
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .to_string();
            map.insert(key.to_string(), value);
        }
    }
    // No closing fence: treat the whole file as body, no frontmatter.
    (map, normalized.to_string())
}

/// Collect `*.md` files under `dir` recursively, sorted by path.
pub(crate) fn collect_markdown(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if !is_plain_dir(dir) {
        return out;
    }
    collect_markdown_into(dir, &mut out);
    out.sort();
    out
}

/// True for a real directory, not a symlink to one.
pub(crate) fn is_plain_dir(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|m| m.file_type().is_dir())
        .unwrap_or(false)
}

/// True for a regular file, not a symlink.
pub(crate) fn is_plain_file(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|m| m.file_type().is_file())
        .unwrap_or(false)
}

/// Canonical `path` must resolve under canonical `root` (catches symlinked
/// intermediate directories).
pub(crate) fn stays_within(root: &Path, path: &Path) -> bool {
    match (fs::canonicalize(root), fs::canonicalize(path)) {
        (Ok(root), Ok(path)) => path.starts_with(root),
        _ => false,
    }
}

fn collect_markdown_into(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if is_plain_dir(&path) {
            collect_markdown_into(&path, out);
        } else if is_plain_file(&path) && path.extension().map(|e| e == "md").unwrap_or(false) {
            out.push(path);
        }
    }
}

/// Plugin display name (namespace) for a source record: the manifest name
/// when present, else the repository name, else the source id.
pub(crate) fn plugin_namespace(record: &PluginSourceRecord) -> String {
    if let Some(name) = record.manifest.plugin.as_ref().map(|p| p.name.as_str()) {
        if is_safe_namespace(name) {
            return name.to_string();
        }
    }
    if let Ok(spec) =
        crate::services::skill_service::github::parse_github_repo_url(&record.manifest.source_url)
    {
        if is_safe_namespace(&spec.repo) {
            return spec.repo;
        }
    }
    record.manifest.source_id.clone()
}

fn is_safe_namespace(name: &str) -> bool {
    !name.trim().is_empty() && !name.contains([':', '/', '\\'])
}
