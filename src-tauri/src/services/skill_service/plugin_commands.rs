//! Plugin slash commands (#2922).
//!
//! A Claude Code plugin may ship Markdown files under `commands/` (or the
//! manifest `commands` paths, #2921). Each file becomes a slash command: the
//! file stem is the command name, the frontmatter `description` /
//! `argument-hint` describe it, and the Markdown body is the prompt template
//! ($ARGUMENTS / $1..$9 substitution).
//!
//! Commands live with the plugin source (global or session store), so they
//! are read-only by construction and an uninstall removes them with the
//! plugin. Session-scoped plugins shadow a global plugin with the same
//! `source_id` for that session only (same rule as #2825 hooks/skills).
//!
//! Names: every command is offered as `/<plugin>:<name>`; the bare
//! `/<name>` is offered only when exactly one effective plugin provides it.
//! A plugin command can never replace a built-in command.

use super::plugin_manifest::{load_plugin_manifest, PluginManifest};
use super::session_plugins::PluginScope;
use crate::services::hooks::registry::{
    get_user_plugins_directory, list_sources_in, PluginSourceRecord,
};
use log::warn;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

/// Built-in command names that a plugin may never shadow.
const BUILTIN_COMMAND_NAMES: [&str; 4] = ["clear", "reload", "permission", "plugin"];

/// One offerable slash command for the chat input dropdown.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginCommandInfo {
    /// The exact text to type, e.g. `/hello` or `/superpowers:hello`.
    pub label: String,
    pub name: String,
    pub plugin_name: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub argument_hint: Option<String>,
    pub scope: PluginScope,
}

/// A command file resolved to its name and frontmatter metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredCommand {
    pub name: String,
    pub path: PathBuf,
    pub description: String,
    pub argument_hint: Option<String>,
}

/// Names of the slash commands a plugin directory provides (preview-time,
/// sorted, file stems). Used by the install preview (#2922 step 1).
pub fn plugin_command_names(root: &Path, manifest: Option<&PluginManifest>) -> Vec<String> {
    let mut names: Vec<String> = discover_command_files(root, manifest)
        .into_iter()
        .map(|command| command.name)
        .collect();
    names.sort();
    names.dedup();
    names
}

/// Resolve every `commands/**/*.md` (default layout) or manifest-declared
/// path inside a plugin root, parsed for frontmatter metadata.
pub fn discover_command_files(
    root: &Path,
    manifest: Option<&PluginManifest>,
) -> Vec<DiscoveredCommand> {
    let files = match manifest.and_then(|m| m.commands.as_ref()) {
        Some(paths) => {
            let mut files = Vec::new();
            for raw in paths.to_vec() {
                let inner = raw.strip_prefix("./").unwrap_or(&raw);
                let target = root.join(inner);
                if is_plain_dir(&target) {
                    files.extend(collect_markdown(&target));
                } else if is_plain_file(&target)
                    && target.extension().map(|e| e == "md").unwrap_or(false)
                {
                    files.push(target);
                }
            }
            files
        }
        None => collect_markdown(&root.join("commands")),
    };

    let mut commands: Vec<DiscoveredCommand> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for path in files {
        // Plugin content is untrusted: never follow a symlink out of the
        // plugin root (a link to a private file would be sent to the LLM).
        if !is_plain_file(&path) || !stays_within(root, &path) {
            warn!(
                "Skipping plugin command outside plugin root: {}",
                path.display()
            );
            continue;
        }
        let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if seen.iter().any(|n| n.eq_ignore_ascii_case(name)) {
            continue;
        }
        seen.push(name.to_string());
        let raw = match fs::read_to_string(&path) {
            Ok(raw) => raw,
            Err(error) => {
                warn!(
                    "Skipping unreadable plugin command {}: {error}",
                    path.display()
                );
                continue;
            }
        };
        let (frontmatter, _body) = split_frontmatter(&raw);
        commands.push(DiscoveredCommand {
            name: name.to_string(),
            path,
            description: frontmatter.get("description").cloned().unwrap_or_default(),
            argument_hint: frontmatter.get("argument-hint").cloned(),
        });
    }
    commands
}

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

/// List every slash command the chat input should offer for a session.
///
/// Session-scoped plugin stores shadow a global plugin with the same
/// `source_id`. Each command always yields the namespaced `/<plugin>:<name>`
/// label; the bare `/<name>` is added only when the name is unique across
/// effective plugins and does not collide with a built-in.
pub fn list_plugin_commands(session_id: &str) -> Result<Vec<PluginCommandInfo>, String> {
    let global_dir = get_user_plugins_directory()?;
    // #2942 gatekeeper G1.3: one session plugin read path — load_session_plugins
    // resolves the store manifest with the out-of-workspace trust overlay, so
    // command discovery sees exactly the records the hook/skill loaders see.
    let session_records = super::session_plugins::load_session_plugins(session_id)?;
    Ok(list_in_dirs(&global_dir, session_records))
}

fn list_in_dirs(
    global_dir: &Path,
    session_records: Vec<PluginSourceRecord>,
) -> Vec<PluginCommandInfo> {
    let mut records: Vec<(PluginSourceRecord, PluginScope)> = Vec::new();
    // Session plugins load through the store manifest (#2942); broken or
    // deleted entries are skipped with a log line, never an error.
    let shadowed: Vec<String> = session_records
        .iter()
        .map(|r| r.manifest.source_id.clone())
        .collect();
    for record in session_records {
        // #2927: a disabled plugin offers no commands, in any scope.
        if !record.manifest.enabled {
            continue;
        }
        records.push((record, PluginScope::Session));
    }
    let global_records = list_sources_in(global_dir).unwrap_or_else(|error| {
        warn!("global plugin store {}: {error}", global_dir.display());
        Vec::new()
    });
    for record in global_records {
        if shadowed.contains(&record.manifest.source_id) || !record.manifest.enabled {
            continue;
        }
        records.push((record, PluginScope::Global));
    }

    // Per-plugin command files (session plugins first for deterministic
    // bare-name resolution when counts tie is impossible: duplicates are
    // rejected below unless the count is exactly one).
    let mut per_plugin: Vec<(String, PluginScope, Vec<DiscoveredCommand>)> = Vec::new();
    for (record, scope) in &records {
        let manifest = manifest_for_discovery(record);
        let commands = discover_command_files(&record.root_dir, manifest.as_ref());
        if commands.is_empty() {
            continue;
        }
        per_plugin.push((plugin_namespace(record), *scope, commands));
    }

    let mut namespaced_count: Vec<(String, usize)> = Vec::new();
    for (_, _, commands) in &per_plugin {
        for command in commands {
            match namespaced_count
                .iter_mut()
                .find(|(name, _)| name.eq_ignore_ascii_case(&command.name))
            {
                Some((_, count)) => *count += 1,
                None => namespaced_count.push((command.name.clone(), 1)),
            }
        }
    }

    let mut out: Vec<PluginCommandInfo> = Vec::new();
    for (plugin_name, scope, commands) in &per_plugin {
        for command in commands {
            let info = PluginCommandInfo {
                label: format!("/{plugin_name}:{}", command.name),
                name: command.name.clone(),
                plugin_name: plugin_name.clone(),
                description: command.description.clone(),
                argument_hint: command.argument_hint.clone(),
                scope: *scope,
            };
            let unique = namespaced_count
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(&command.name))
                .map(|(_, count)| *count == 1)
                .unwrap_or(false);
            let builtin_collision = BUILTIN_COMMAND_NAMES
                .iter()
                .any(|b| b.eq_ignore_ascii_case(&command.name));
            if unique && !builtin_collision {
                let mut bare = info.clone();
                bare.label = format!("/{}", command.name);
                out.push(bare);
            }
            out.push(info);
        }
    }
    out.sort_by(|a, b| a.label.cmp(&b.label));
    out
}

/// Re-resolve custom `commands` paths from the on-disk
/// `.claude-plugin/plugin.json` when one survived install; otherwise default
/// `commands/` discovery. The install record persists only display
/// metadata (`PluginMetadata`), so discovery reads the manifest from disk.
pub(crate) fn manifest_for_discovery(record: &PluginSourceRecord) -> Option<PluginManifest> {
    load_plugin_manifest(&record.root_dir).ok().flatten()
}

/// Expand `/name [args]` or `/plugin:name [args]` into the command body with
/// `$ARGUMENTS` and `$1..$9` substituted. `Ok(None)` means no plugin command
/// matches (the caller keeps its "unknown command" behavior).
pub fn expand_plugin_command(
    session_id: &str,
    command_text: &str,
) -> Result<Option<String>, String> {
    let global_dir = get_user_plugins_directory()?;
    // #2942 gatekeeper G1.3: one session plugin read path (see list_plugin_commands).
    let session_records = super::session_plugins::load_session_plugins(session_id)?;
    Ok(expand_in_dirs(&global_dir, session_records, command_text))
}

fn expand_in_dirs(
    global_dir: &Path,
    session_records: Vec<PluginSourceRecord>,
    command_text: &str,
) -> Option<String> {
    let trimmed = command_text.trim();
    let head = trimmed.strip_prefix('/')?;
    if head.is_empty()
        || head
            .chars()
            .next()
            .map(|c| c.is_whitespace())
            .unwrap_or(true)
    {
        return None;
    }
    let (token, args) = match head.split_once(char::is_whitespace) {
        Some((token, args)) => (token, args.trim()),
        None => (head, ""),
    };
    if BUILTIN_COMMAND_NAMES.iter().any(|b| {
        token.eq_ignore_ascii_case(b)
            || token
                .split(':')
                .next()
                .unwrap_or(token)
                .eq_ignore_ascii_case(b)
    }) {
        return None;
    }

    let records = effective_records(global_dir, session_records);
    let (plugin_filter, command_name) = match token.split_once(':') {
        Some((plugin, name)) => (Some(plugin), name),
        None => (None, token),
    };

    let mut candidate_bodies: Vec<(String, DiscoveredCommand)> = Vec::new();
    for record in &records {
        let namespace = plugin_namespace(record);
        if let Some(plugin) = plugin_filter {
            if !namespace.eq_ignore_ascii_case(plugin) {
                continue;
            }
        }
        let manifest = manifest_for_discovery(record);
        for command in discover_command_files(&record.root_dir, manifest.as_ref()) {
            if command.name.eq_ignore_ascii_case(command_name) {
                candidate_bodies.push((namespace.clone(), command));
            }
        }
    }

    match candidate_bodies.len() {
        0 => None,
        1 => {
            let (_, command) = candidate_bodies.pop().unwrap();
            if !is_plain_file(&command.path) {
                return None;
            }
            let raw = fs::read_to_string(&command.path).ok()?;
            let (_frontmatter, body) = split_frontmatter(&raw);
            Some(substitute_arguments(body.trim(), args))
        }
        _ => {
            // Bare-name ambiguity across distinct plugins: namespaced form
            // is the disambiguator. Same-plugin duplicates cannot happen
            // (discovery dedups by name).
            let plugins: Vec<String> = candidate_bodies
                .iter()
                .map(|(namespace, _)| namespace.clone())
                .collect();
            warn!(
                "Slash command /{command_name} is ambiguous across plugins {:?}",
                plugins
            );
            None
        }
    }
}

fn effective_records(
    global_dir: &Path,
    mut records: Vec<PluginSourceRecord>,
) -> Vec<PluginSourceRecord> {
    let shadowed: Vec<String> = records
        .iter()
        .map(|r| r.manifest.source_id.clone())
        .collect();
    records.extend(
        list_sources_in(global_dir)
            .unwrap_or_default()
            .into_iter()
            .filter(|r| !shadowed.contains(&r.manifest.source_id)),
    );
    records
}

/// Replace `$ARGUMENTS` with the full argument string and `$1..$9` with
/// positional arguments (missing positions expand to empty). A single scan
/// keeps `$10` from being eaten as `$1` + `0`.
fn substitute_arguments(body: &str, args: &str) -> String {
    let positional: Vec<&str> = args.split_whitespace().collect();
    let mut out = String::with_capacity(body.len() + args.len());
    let bytes = body.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'$' {
            let rest = &body[i + 1..];
            if rest.starts_with("ARGUMENTS") {
                out.push_str(args);
                i += "$ARGUMENTS".len();
                continue;
            }
            if let Some(index) = rest.chars().next().and_then(|c| c.to_digit(10)) {
                if (1..=9).contains(&index) {
                    out.push_str(positional.get(index as usize - 1).unwrap_or(&""));
                    i += 2;
                    continue;
                }
            }
        }
        let ch = body[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::hooks::registry::{
        list_session_store_sources, save_source_in, upsert_session_store_entry,
        PluginSourceManifest, PluginSourceRecord,
    };
    use std::collections::BTreeMap;

    fn manifest(source_id: &str, source_url: &str) -> PluginSourceManifest {
        PluginSourceManifest {
            source_url: source_url.to_string(),
            source_id: source_id.to_string(),
            skill_names: vec![],
            hooks: None,
            hooks_approved: false,
            approval_hashes: BTreeMap::new(),
            created_at: 0,
            plugin: None,
            hook_files: vec![],
            mcp_servers: vec![],
            mcp_approved: false,
            enabled: true,
            installed_commit: None,
        }
    }

    /// `source_id` must pass `validate_source_id`: `plugin-` + 16 hex chars.
    fn source_id(n: u8) -> String {
        format!("plugin-{:016x}", n as u128)
    }

    fn install(dir: &Path, id: &str, url: &str, files: &[(&str, &str)]) -> PluginSourceRecord {
        let record = save_source_in(dir, manifest(id, url)).unwrap();
        // Session stores load through the store manifest (#2942); keep the
        // helper usable for both global and session dirs (the manifest is
        // only ever read from session stores).
        upsert_session_store_entry(
            dir,
            crate::services::hooks::registry::SessionPluginStoreEntry {
                source_id: id.to_string(),
                source_url: url.to_string(),
                installed_at: 0,
            },
        )
        .unwrap();
        for (rel, content) in files {
            let path = record.root_dir.join(rel);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, content).unwrap();
        }
        record
    }

    const HELLO: &str =
        "---\ndescription: Say hello\nargument-hint: <name>\n---\nHello $ARGUMENTS!\n";

    /// #2927 fixture: an installed plugin that the user has switched off.
    fn install_disabled(
        dir: &Path,
        id: &str,
        url: &str,
        files: &[(&str, &str)],
    ) -> PluginSourceRecord {
        install(dir, id, url, files);
        crate::services::hooks::registry::set_source_enabled_in(dir, id, false).unwrap()
    }

    /// #2927: a disabled plugin offers no commands in either scope, while an
    /// enabled neighbour keeps working.
    #[test]
    fn disabled_plugin_offers_no_commands() {
        let global = tempfile::tempdir().unwrap();
        let session = tempfile::tempdir().unwrap();
        install(
            global.path(),
            &source_id(6),
            "https://github.com/acme/keep",
            &[("commands/keep.md", "---\ndescription: keep\n---\nKeep\n")],
        );
        install_disabled(
            global.path(),
            &source_id(7),
            "https://github.com/acme/dropped",
            &[(
                "commands/dropped.md",
                "---\ndescription: dropped\n---\nDropped\n",
            )],
        );
        let session_disabled = install_disabled(
            session.path(),
            &source_id(8),
            "https://github.com/acme/sessiondropped",
            &[(
                "commands/sessiondropped.md",
                "---\ndescription: s\n---\nS\n",
            )],
        );

        let out = list_in_dirs(global.path(), vec![session_disabled]);
        let names: Vec<&str> = out.iter().map(|c| c.name.as_str()).collect();
        assert!(
            names.contains(&"keep"),
            "enabled plugin commands: {names:?}"
        );
        assert!(
            !names.iter().any(|name| name.contains("dropped")),
            "disabled plugins must offer no commands: {names:?}"
        );
        assert!(
            out.iter().all(|command| command.name == "keep"),
            "only the enabled command survives: {out:?}"
        );
    }

    #[test]
    fn discovers_default_commands_dir() {
        let dir = tempfile::tempdir().unwrap();
        let record = install(
            dir.path(),
            &source_id(1),
            "https://github.com/acme/hello",
            &[
                ("commands/hello.md", HELLO),
                (
                    "commands/nested/help.md",
                    "---\ndescription: nested help\n---\nBody\n",
                ),
            ],
        );
        let commands = discover_command_files(&record.root_dir, None);
        let names: Vec<&str> = commands.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"hello"), "names: {names:?}");
        assert!(names.contains(&"help"), "recursive discovery: {names:?}");
        let hello = commands.iter().find(|c| c.name == "hello").unwrap();
        assert_eq!(hello.description, "Say hello");
        assert_eq!(hello.argument_hint.as_deref(), Some("<name>"));
    }

    #[test]
    fn plugin_command_names_for_preview() {
        let dir = tempfile::tempdir().unwrap();
        install(
            dir.path(),
            &source_id(2),
            "https://github.com/acme/p",
            &[
                ("commands/zebra.md", "z"),
                ("commands/alpha.md", "a"),
                ("skills/x/SKILL.md", "s"),
            ],
        );
        let names = plugin_command_names(&dir.path().join(source_id(2)), None);
        assert_eq!(names, vec!["alpha".to_string(), "zebra".to_string()]);
    }

    #[test]
    fn bare_label_only_when_unique_and_never_shadows_builtin() {
        let dir = tempfile::tempdir().unwrap();
        let global = dir.path().join("global");
        let session = dir.path().join("session");
        install(
            &global,
            &source_id(3),
            "https://github.com/acme/a",
            &[
                ("commands/hello.md", HELLO),
                ("commands/clear.md", "b"),
                ("commands/dup.md", "d"),
            ],
        );
        install(
            &global,
            &source_id(4),
            "https://github.com/acme/b",
            &[("commands/dup.md", "d2")],
        );
        let out = list_in_dirs(&global, list_session_store_sources(None, &session));
        let labels: Vec<&str> = out.iter().map(|c| c.label.as_str()).collect();
        assert!(labels.contains(&"/hello"), "{labels:?}");
        assert!(labels.contains(&"/a:hello"), "{labels:?}");
        // Built-in collision: namespaced form allowed, bare form never.
        assert!(labels.contains(&"/a:clear"), "{labels:?}");
        assert!(
            !labels.contains(&"/clear"),
            "built-in shadowing: {labels:?}"
        );
        // Ambiguous across plugins: no bare form.
        assert!(!labels.contains(&"/dup"), "ambiguous bare: {labels:?}");
        assert!(
            labels.contains(&"/a:dup") || labels.contains(&"/b:dup"),
            "{labels:?}"
        );
    }

    #[test]
    fn session_plugin_shadows_global_same_source_id() {
        let dir = tempfile::tempdir().unwrap();
        let global = dir.path().join("global");
        let session = dir.path().join("session");
        install(
            &global,
            &source_id(5),
            "https://github.com/acme/shadow",
            &[("commands/hello.md", "global body")],
        );
        install(
            &session,
            &source_id(5),
            "https://github.com/acme/shadow",
            &[("commands/hello.md", HELLO)],
        );
        let out = list_in_dirs(&global, list_session_store_sources(None, &session));
        let hello = out.iter().filter(|c| c.name == "hello").collect::<Vec<_>>();
        // Shadowed source appears once (session), so the bare label survives.
        assert_eq!(hello.len(), 2, "bare + namespaced: {out:?}");
        assert!(hello.iter().all(|c| c.scope == PluginScope::Session));
    }

    #[test]
    fn substitution_handles_all_forms() {
        assert_eq!(substitute_arguments("Hi $ARGUMENTS!", "a b c"), "Hi a b c!");
        assert_eq!(substitute_arguments("$1 and $2", "x y"), "x and y");
        assert_eq!(substitute_arguments("$1", ""), "");
        // $10 is $1 followed by a literal 0, not a tenth positional.
        assert_eq!(substitute_arguments("$10", "x"), "x0");
        assert_eq!(
            substitute_arguments("no placeholders", "x"),
            "no placeholders"
        );
        assert_eq!(substitute_arguments("$ARGUMENTS $ARGUMENTS", "r"), "r r");
    }

    #[test]
    fn expansion_reads_body_and_args() {
        let dir = tempfile::tempdir().unwrap();
        let global = dir.path().join("global");
        let session = dir.path().join("session");
        install(
            &global,
            &source_id(6),
            "https://github.com/acme/e",
            &[("commands/hello.md", HELLO)],
        );
        let expanded = expand_in_dirs(
            &global,
            list_session_store_sources(None, &session),
            "/hello world",
        )
        .unwrap();
        assert_eq!(expanded, "Hello world!");
        let namespaced = expand_in_dirs(
            &global,
            list_session_store_sources(None, &session),
            "/e:hello world",
        )
        .unwrap();
        assert_eq!(namespaced, "Hello world!");
        assert!(
            expand_in_dirs(&global, list_session_store_sources(None, &session), "/nope").is_none()
        );
        // Built-ins are never expanded by plugin resolution.
        assert!(expand_in_dirs(
            &global,
            list_session_store_sources(None, &session),
            "/clear"
        )
        .is_none());
    }

    #[test]
    fn frontmatter_without_fence_is_whole_body() {
        let (map, body) = split_frontmatter("plain body\n");
        assert!(map.is_empty());
        assert_eq!(body, "plain body\n");
        let (map, body) = split_frontmatter("---\ndescription: d\n---\nBody text\n");
        assert_eq!(map.get("description").map(String::as_str), Some("d"));
        assert_eq!(body, "Body text\n");
    }

    #[test]
    fn frontmatter_body_keeps_inner_rules_and_crlf() {
        let (map, body) = split_frontmatter("---\ndescription: a --- b\n---\nBody\n---\nMore\n");
        assert_eq!(map.get("description").map(String::as_str), Some("a --- b"));
        assert_eq!(body, "Body\n---\nMore\n");
        let (map, body) =
            split_frontmatter("---\r\ndescription: d\r\n---\r\nBody\r\n---\r\nMore\r\n");
        assert_eq!(map.get("description").map(String::as_str), Some("d"));
        assert_eq!(body, "Body\r\n---\r\nMore\r\n");
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_files_and_dirs_are_not_discovered() {
        use std::os::unix::fs::symlink;
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("secret.md"), "SECRET").unwrap();
        fs::create_dir_all(outside.path().join("dir")).unwrap();
        fs::write(outside.path().join("dir/inner.md"), "SECRET").unwrap();

        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("commands")).unwrap();
        fs::write(root.path().join("commands/ok.md"), "fine").unwrap();
        symlink(
            outside.path().join("secret.md"),
            root.path().join("commands/notes.md"),
        )
        .unwrap();
        symlink(
            outside.path().join("dir"),
            root.path().join("commands/linked"),
        )
        .unwrap();

        let names = plugin_command_names(root.path(), None);
        assert_eq!(names, vec!["ok".to_string()]);

        // A symlinked commands/ directory yields nothing at all.
        let root2 = tempfile::tempdir().unwrap();
        symlink(outside.path().join("dir"), root2.path().join("commands")).unwrap();
        assert!(plugin_command_names(root2.path(), None).is_empty());
    }

    #[test]
    fn namespace_prefers_manifest_then_repo_name() {
        let dir = tempfile::tempdir().unwrap();
        let mut m = manifest(&source_id(7), "https://github.com/obra/superpowers");
        m.plugin = Some(crate::services::hooks::registry::PluginMetadata {
            name: "superpowers".into(),
            version: None,
            description: None,
            author: None,
            homepage: None,
            repository: None,
            license: None,
            keywords: vec![],
        });
        let record = save_source_in(dir.path(), m).unwrap();
        assert_eq!(plugin_namespace(&record), "superpowers");
        let record = install(
            dir.path(),
            &source_id(8),
            "https://github.com/acme/repo",
            &[],
        );
        assert_eq!(plugin_namespace(&record), "repo");
    }
}
