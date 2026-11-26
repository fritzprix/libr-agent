use super::super::schema::{HookEvent, HookHandler, HooksConfig};
use super::sources::{get_source, list_sources, list_sources_in, save_source};
use super::store::list_session_store_sources;
use super::{PluginSourceManifest, PluginSourceRecord};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

pub fn set_hooks_approved(
    source_id: &str,
    approved: bool,
    approval_hashes: BTreeMap<String, String>,
) -> Result<PluginSourceManifest, String> {
    let mut record = get_source(source_id)?;
    record.manifest.hooks_approved = approved;
    record.manifest.approval_hashes = approval_hashes;
    let saved = save_source(record.manifest)?;
    Ok(saved.manifest)
}

pub fn compute_script_hash(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let digest = Sha256::digest(bytes);
    Ok(format!("{:x}", digest))
}

pub fn compute_handler_approval_hashes(
    plugin_root: &Path,
    config: &HooksConfig,
) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for (event, groups) in &config.events {
        for group in groups {
            for handler in &group.handlers {
                let key = approval_hash_key(*event, group.matcher.as_deref(), handler);
                map.insert(key, approval_hash_value(plugin_root, handler));
            }
        }
    }
    map
}

pub fn approval_hash_key(event: HookEvent, matcher: Option<&str>, handler: &HookHandler) -> String {
    let args_suffix = handler
        .args
        .as_ref()
        .map(|args| format!("|args:{}", args.join("\u{1}")))
        .unwrap_or_default();
    let mut key = format!(
        "{}|{}|{}|{}{}",
        event.as_str(),
        matcher.unwrap_or("*"),
        handler.command,
        handler.powershell.as_deref().unwrap_or(""),
        args_suffix
    );
    // HTTP handlers carry no command, so the target URL distinguishes them
    // (#3009). Appended only when present, so keys of command handlers stay
    // byte-identical to earlier installs. Headers and allowed env-var names
    // are part of what the handler sends, so they must distinguish the key
    // too — otherwise two handlers that differ only by headers share one
    // key and one is skipped as "unapproved or modified" (#3041 G1.4).
    if let Some(http) = &handler.http {
        key.push_str("|url=");
        key.push_str(&http.url);
        for (name, value) in &http.headers {
            key.push_str("|header=");
            key.push_str(name);
            key.push('=');
            key.push_str(value);
        }
        for name in &http.allowed_env_vars {
            key.push_str("|env=");
            key.push_str(name);
        }
    }
    let digest = Sha256::digest(key.as_bytes());
    format!("{:x}", digest)
}

pub fn approval_hash_value(plugin_root: &Path, handler: &HookHandler) -> String {
    let mut hasher = Sha256::new();
    hasher.update(handler.command.as_bytes());
    hasher.update(b"\0");
    if let Some(ps) = &handler.powershell {
        hasher.update(ps.as_bytes());
    }
    hasher.update(b"\0");
    if let Some(shell) = &handler.shell {
        hasher.update(shell.as_bytes());
    }
    // The URL, header names, and allowed env-var names are part of what an
    // HTTP handler does, so they are part of what the user approved (#3009).
    if let Some(http) = &handler.http {
        hasher.update(b"\0url=");
        hasher.update(http.url.as_bytes());
        for (name, value) in &http.headers {
            hasher.update(b"\0header\0");
            hasher.update(name.as_bytes());
            hasher.update(b"=");
            hasher.update(value.as_bytes());
        }
        for var in &http.allowed_env_vars {
            hasher.update(b"\0env\0");
            hasher.update(var.as_bytes());
        }
    }
    // Exec-form arguments are part of what runs, so they are part of what the
    // user approved (#3005). Only appended when present, so a handler without
    // `args` keeps the hash an earlier install recorded.
    if let Some(args) = &handler.args {
        hasher.update(b"\0args\0");
        for arg in args {
            hasher.update(arg.as_bytes());
            hasher.update(b"\0");
        }
    }
    for (rel, file_hash) in collect_referenced_script_hashes(plugin_root, handler) {
        hasher.update(b"\0");
        hasher.update(rel.as_bytes());
        hasher.update(b"=");
        hasher.update(file_hash.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

fn collect_referenced_script_hashes(
    plugin_root: &Path,
    handler: &HookHandler,
) -> BTreeMap<String, String> {
    let mut hashes = BTreeMap::new();
    let canonical_root = plugin_root
        .canonicalize()
        .unwrap_or_else(|_| plugin_root.to_path_buf());
    let root_str = plugin_root.to_string_lossy();

    let expand = |text: &str| {
        text.replace("${LIBRAGENT_PLUGIN_ROOT}", root_str.as_ref())
            .replace("${CLAUDE_PLUGIN_ROOT}", root_str.as_ref())
            .replace("${CURSOR_PLUGIN_ROOT}", root_str.as_ref())
            .replace("$LIBRAGENT_PLUGIN_ROOT", root_str.as_ref())
            .replace("$CLAUDE_PLUGIN_ROOT", root_str.as_ref())
            .replace("$CURSOR_PLUGIN_ROOT", root_str.as_ref())
    };
    let record_candidate = |hashes: &mut BTreeMap<String, String>, tok: &str| {
        let candidate = if Path::new(tok).is_absolute() {
            PathBuf::from(tok)
        } else {
            plugin_root.join(tok)
        };
        if !candidate.is_file() {
            return;
        }
        let Ok(canonical_file) = candidate.canonicalize() else {
            return;
        };
        let Ok(rel) = canonical_file.strip_prefix(&canonical_root) else {
            return;
        };
        if let Ok(file_hash) = compute_script_hash(&canonical_file) {
            let rel_key = rel.to_string_lossy().replace('\\', "/");
            hashes.insert(rel_key, file_hash);
        }
    };

    let mut texts = vec![handler.command.as_str()];
    if let Some(ps) = handler.powershell.as_deref() {
        texts.push(ps);
    }

    for text in texts {
        let expanded = expand(text);
        for raw_tok in expanded.split_whitespace() {
            let tok = raw_tok.trim_matches(|c| matches!(c, '"' | '\'' | ';' | ','));
            if tok.is_empty() {
                continue;
            }
            record_candidate(&mut hashes, tok);
        }
    }

    // Exec form (`args` present, #3005): each argument is passed verbatim to
    // the spawned command, so a script path in argv runs exactly as written —
    // hash each element whole. Without this, `"command": "node",
    // "args": ["${CLAUDE_PLUGIN_ROOT}/x.js"]` would leave x.js unhashed and
    // editable after approval (gatekeeper G1.1). A whole argument is not
    // enough either: `"bash", ["-c", "./hooks/x.sh --fix"]` runs a script
    // wrapped inside a command string, so the path tokens inside each
    // argument are hashed too, exactly as for shell-form text (G2.2).
    if let Some(args) = handler.args.as_deref() {
        for arg in args {
            let expanded = expand(arg);
            if expanded.trim().is_empty() {
                continue;
            }
            record_candidate(&mut hashes, expanded.trim());
            for raw_tok in expanded.split_whitespace() {
                let tok = raw_tok.trim_matches(|c| matches!(c, '"' | '\'' | ';' | ','));
                if tok.is_empty() {
                    continue;
                }
                record_candidate(&mut hashes, tok);
            }
        }
    }
    hashes
}

pub fn list_approved_sources_in(base: &Path) -> Vec<PluginSourceRecord> {
    list_sources_in(base)
        .unwrap_or_default()
        .into_iter()
        .filter(|r| r.manifest.hooks_approved && r.manifest.enabled)
        .collect()
}

/// Approved plugin sources effective for one session: the session's own
/// installs plus global installs that the session has not shadowed.
///
/// A session install wins over a global install with the same `source_id`
/// even when the session copy is not hook-approved, so a plugin's hooks never
/// run twice and a session can opt out of a global plugin's hooks (#2825).
pub fn list_effective_approved_sources_in(
    global_base: &Path,
    session_base: Option<&Path>,
    session_trust_dir: Option<&Path>,
) -> Vec<PluginSourceRecord> {
    let session_sources = match session_base {
        Some(base) => list_session_store_sources(session_trust_dir, base),
        None => Vec::new(),
    };
    let shadowed: HashSet<String> = session_sources
        .iter()
        .map(|r| r.manifest.source_id.clone())
        .collect();
    let mut out: Vec<PluginSourceRecord> = list_approved_sources_in(global_base)
        .into_iter()
        .filter(|r| !shadowed.contains(&r.manifest.source_id))
        .collect();
    out.extend(
        session_sources
            .into_iter()
            .filter(|r| r.manifest.hooks_approved && r.manifest.enabled),
    );
    out
}

pub fn list_approved_plugin_roots() -> Vec<PathBuf> {
    list_sources()
        .unwrap_or_default()
        .into_iter()
        .filter(|r| r.manifest.hooks_approved && r.manifest.enabled)
        .map(|r| r.root_dir)
        .collect()
}
