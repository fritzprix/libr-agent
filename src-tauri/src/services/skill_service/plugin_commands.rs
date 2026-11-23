//! Plugin slash commands (#2922).
//!
//! A Claude Code plugin ships prompt templates under `commands/**/*.md`
//! (Markdown with optional YAML frontmatter). LibrAgent keeps the templates
//! inside the installed plugin source and resolves them on demand, so
//! uninstalling a plugin removes its commands with it. Commands never enter
//! an editable command store — that is what makes the #2920 D2 ownership
//! rule trivially true: there is nothing to edit, delete, or import over.
//!
//! Invocation rules:
//! - Every command is always reachable as `/<plugin-name>:<command>`.
//! - A bare `/<command>` form exists only when no other plugin shares the
//!   name (case-insensitive) and the name is not a built-in.
//! - Built-in parser commands (`/clear`, `/reload`, `/permission`, `/plugin`)
//!   always win; a plugin can never shadow them (criterion 5).

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::contracts::PluginCommandPreview;
use super::session_plugins::PluginScope;
use crate::services::hooks::registry::{
    get_session_plugins_directory, get_user_plugins_directory, list_sources_in, PluginSourceRecord,
};

/// Directory inside a plugin root that holds slash-command templates.
pub const PLUGIN_COMMANDS_DIR: &str = "commands";

/// Invocation tokens owned by the built-in parser (`agent/command_parser.rs`).
/// A plugin command may never take one of these over (criterion 5).
const RESERVED_COMMAND_TOKENS: [&str; 4] = ["clear", "reload", "permission", "plugin"];

/// One plugin command as offered to the chat input and the expand IPC.
#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PluginCommandInfo {
    /// Plugin name (`.claude-plugin/plugin.json` `name`, falling back to the
    /// source id when the plugin carried no manifest).
    pub plugin: String,
    /// Command name relative to `commands/` (`git:commit` for
    /// `commands/git/commit.md`).
    pub command: String,
    /// Always-available namespaced form, e.g. `/myplugin:hello`.
    pub invocation: String,
    /// Bare form `/hello`, present only when no other plugin command shares
    /// the name and the name is not a built-in.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bare_invocation: Option<String>,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub argument_hint: Option<String>,
    pub scope: PluginScope,
    pub source_id: String,
    /// Absolute path of the Markdown template on disk.
    #[serde(skip)]
    pub file_path: String,
}

#[derive(Debug, Default, Deserialize)]
struct CommandFrontmatter {
    #[serde(default)]
    description: Option<String>,
    #[serde(default, rename = "argument-hint")]
    argument_hint: Option<String>,
}

fn clean(value: Option<String>) -> Option<String> {
    value.filter(|v| !v.trim().is_empty())
}

/// Splits `---`-delimited YAML frontmatter from the Markdown body.
/// Returns `(frontmatter_yaml, body)`, or `(None, whole content)` when the
/// file has no frontmatter block.
fn split_frontmatter(content: &str) -> (Option<&str>, &str) {
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);
    if let Some(rest) = content.strip_prefix("---") {
        if let Some(end_idx) = rest.find("---") {
            let frontmatter = &rest[..end_idx];
            let body = rest[end_idx + 3..].trim_start_matches(['\r', '\n']);
            return (Some(frontmatter), body);
        }
    }
    (None, content)
}

/// Parses command frontmatter leniently: a missing or broken frontmatter
/// block degrades to no metadata instead of hiding the command.
fn parse_command_frontmatter(content: &str) -> (CommandFrontmatter, &str) {
    match split_frontmatter(content) {
        (Some(raw), body) => (
            serde_yaml::from_str::<CommandFrontmatter>(raw).unwrap_or_default(),
            body,
        ),
        (None, body) => (CommandFrontmatter::default(), body),
    }
}

/// Recursively collects `**/*.md` under `dir` as `(name, path)`, where `name`
/// is the file path relative to `dir` with separators collapsed to `:` and
/// the `.md` suffix removed.
fn collect_command_files(base: &Path, dir: &Path, out: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_command_files(base, &path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
            let rel = path.strip_prefix(base).unwrap_or(path.as_path());
            let rel_str = rel.to_string_lossy().replace('\\', "/");
            let name = rel_str
                .strip_suffix(".md")
                .unwrap_or(rel_str.as_str())
                .replace('/', ":");
            out.push((name, path));
        }
    }
}

/// Scans `root/commands/` for the install preview (#2922). Names here are
/// plugin-local (no plugin prefix); the listing prefixes them later.
pub(crate) fn scan_plugin_commands(root: &Path) -> Vec<PluginCommandPreview> {
    let dir = root.join(PLUGIN_COMMANDS_DIR);
    let mut files = Vec::new();
    collect_command_files(&dir, &dir, &mut files);
    files.sort_by(|a, b| a.0.cmp(&b.0));
    files
        .into_iter()
        .map(|(name, path)| {
            let content = fs::read_to_string(&path).unwrap_or_default();
            let (frontmatter, _) = parse_command_frontmatter(&content);
            PluginCommandPreview {
                name,
                description: clean(frontmatter.description),
                argument_hint: clean(frontmatter.argument_hint),
            }
        })
        .collect()
}

struct CommandEntry {
    plugin: String,
    command: String,
    description: String,
    argument_hint: Option<String>,
    scope: PluginScope,
    source_id: String,
    file_path: PathBuf,
}

fn entries_from_record(record: &PluginSourceRecord, scope: PluginScope) -> Vec<CommandEntry> {
    let plugin = record
        .manifest
        .plugin
        .as_ref()
        .map(|p| p.name.clone())
        .unwrap_or_else(|| record.manifest.source_id.clone());
    let dir = record.root_dir.join(PLUGIN_COMMANDS_DIR);
    let mut files = Vec::new();
    collect_command_files(&dir, &dir, &mut files);
    files.sort_by(|a, b| a.0.cmp(&b.0));
    files
        .into_iter()
        .map(|(command, file_path)| {
            let content = fs::read_to_string(&file_path).unwrap_or_default();
            let (frontmatter, _) = parse_command_frontmatter(&content);
            CommandEntry {
                plugin: plugin.clone(),
                command,
                description: clean(frontmatter.description).unwrap_or_default(),
                argument_hint: clean(frontmatter.argument_hint),
                scope,
                source_id: record.manifest.source_id.clone(),
                file_path,
            }
        })
        .collect()
}

/// Listing over explicit stores (session first; a session source shadows the
/// global source with the same `source_id`, matching `list_session_plugins`).
pub(crate) fn list_plugin_commands_in(
    global_dir: &Path,
    session_dir: Option<&Path>,
) -> Result<Vec<PluginCommandInfo>, String> {
    let mut entries = Vec::new();
    if let Some(dir) = session_dir {
        for record in list_sources_in(dir)? {
            entries.extend(entries_from_record(&record, PluginScope::Session));
        }
    }
    let shadowed: HashSet<String> = entries.iter().map(|e| e.source_id.clone()).collect();
    for record in list_sources_in(global_dir)? {
        if shadowed.contains(&record.manifest.source_id) {
            continue;
        }
        entries.extend(entries_from_record(&record, PluginScope::Global));
    }

    let mut bare_counts: HashMap<String, usize> = HashMap::new();
    for entry in &entries {
        *bare_counts.entry(entry.command.to_lowercase()).or_insert(0) += 1;
    }

    let mut infos: Vec<PluginCommandInfo> = entries
        .into_iter()
        .map(|entry| {
            let lower = entry.command.to_lowercase();
            let bare_ok =
                bare_counts[&lower] == 1 && !RESERVED_COMMAND_TOKENS.contains(&lower.as_str());
            let bare_invocation = bare_ok.then(|| format!("/{}", entry.command));
            PluginCommandInfo {
                invocation: format!("/{}:{}", entry.plugin, entry.command),
                plugin: entry.plugin,
                command: entry.command,
                bare_invocation,
                description: entry.description,
                argument_hint: entry.argument_hint,
                scope: entry.scope,
                source_id: entry.source_id,
                file_path: entry.file_path.to_string_lossy().to_string(),
            }
        })
        .collect();
    infos.sort_by(|a, b| a.invocation.cmp(&b.invocation));
    Ok(infos)
}

/// Resolves `text` (e.g. `/hello args` or `/p:hello args`) against the
/// listing and expands the template body with the arguments. Built-in
/// invocations return `Ok(None)` so the parser in `agent/command_parser.rs`
/// always wins.
pub(crate) fn expand_plugin_command_in(
    global_dir: &Path,
    session_dir: Option<&Path>,
    text: &str,
) -> Result<Option<String>, String> {
    let trimmed = text.trim();
    if !trimmed.starts_with('/') {
        return Ok(None);
    }
    if crate::agent::command_parser::Command::parse(trimmed).is_some() {
        return Ok(None);
    }
    let (token, args) = match trimmed.find(char::is_whitespace) {
        Some(index) => (&trimmed[..index], trimmed[index..].trim()),
        None => (trimmed, ""),
    };

    let infos = list_plugin_commands_in(global_dir, session_dir)?;
    let entry = infos.iter().find(|info| {
        info.invocation.eq_ignore_ascii_case(token)
            || info
                .bare_invocation
                .as_deref()
                .is_some_and(|bare| bare.eq_ignore_ascii_case(token))
    });
    let Some(entry) = entry else {
        return Ok(None);
    };

    let content = fs::read_to_string(&entry.file_path)
        .map_err(|e| format!("read {}: {e}", entry.file_path))?;
    let (_, body) = parse_command_frontmatter(&content);
    Ok(Some(expand_arguments(body.trim_end(), args)))
}

/// `$ARGUMENTS` → the full argument string, `$1`..`$9` → positional
/// arguments. When the template references no placeholder but arguments were
/// given, they are appended (Claude Code behaviour).
fn expand_arguments(body: &str, args: &str) -> String {
    let positional: Vec<&str> = args.split_whitespace().collect();
    let mut out = body.replace("$ARGUMENTS", args);
    for (index, value) in positional.iter().enumerate() {
        out = out.replace(&format!("${}", index + 1), value);
    }
    let used_placeholder =
        body.contains("$ARGUMENTS") || (1..=9).any(|n| body.contains(&format!("${n}")));
    if !args.is_empty() && !used_placeholder {
        out.push_str("\n\n");
        out.push_str(args);
    }
    out
}

/// Slash commands available to one session (global + session plugins).
pub fn list_plugin_commands(session_id: &str) -> Result<Vec<PluginCommandInfo>, String> {
    let global_dir = get_user_plugins_directory()?;
    let session_dir = get_session_plugins_directory(session_id)?;
    list_plugin_commands_in(&global_dir, Some(&session_dir))
}

/// Expands a plugin slash command typed in chat into its prompt body, or
/// `None` when the text is a built-in command or not a plugin command at all.
pub fn expand_plugin_command(session_id: &str, text: &str) -> Result<Option<String>, String> {
    let global_dir = get_user_plugins_directory()?;
    let session_dir = get_session_plugins_directory(session_id)?;
    expand_plugin_command_in(&global_dir, Some(&session_dir), text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::hooks::registry::{save_source_in, PluginMetadata, PluginSourceManifest};
    use std::collections::BTreeMap;

    fn manifest(source_id: &str, plugin_name: Option<&str>) -> PluginSourceManifest {
        PluginSourceManifest {
            source_url: format!("https://example.test/{source_id}"),
            source_id: source_id.to_string(),
            skill_names: vec![],
            hooks: None,
            hooks_approved: false,
            approval_hashes: BTreeMap::new(),
            created_at: 0,
            plugin: plugin_name.map(|name| PluginMetadata {
                name: name.to_string(),
                version: None,
                description: None,
                author: None,
                homepage: None,
                repository: None,
                license: None,
                keywords: vec![],
            }),
            hook_files: vec![],
        }
    }

    /// `validate_source_id` requires `plugin-` + 16 hex chars.
    fn source_id(hex: &str) -> String {
        format!("plugin-{hex:0>16}")
    }

    fn install_plugin(base: &Path, hex: &str, plugin_name: Option<&str>) -> String {
        let id = source_id(hex);
        save_source_in(base, manifest(&id, plugin_name)).unwrap();
        id
    }

    fn write_command(plugin_base: &Path, rel: &str, content: &str) {
        let path = plugin_base.join(PLUGIN_COMMANDS_DIR).join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, content).unwrap();
    }

    fn plugin_root(base: &Path, source_id: &str) -> PathBuf {
        base.join(source_id)
    }

    #[test]
    fn lists_namespaced_and_unique_bare_forms() {
        let global = tempfile::tempdir().unwrap();
        install_plugin(global.path(), "a", Some("alpha"));
        write_command(
            &plugin_root(global.path(), &source_id("a")),
            "hello.md",
            "---\ndescription: say hi\nargument-hint: <name>\n---\nHi $ARGUMENTS\n",
        );
        let infos = list_plugin_commands_in(global.path(), None).unwrap();
        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0].invocation, "/alpha:hello");
        assert_eq!(infos[0].bare_invocation.as_deref(), Some("/hello"));
        assert_eq!(infos[0].description, "say hi");
        assert_eq!(infos[0].argument_hint.as_deref(), Some("<name>"));
        assert_eq!(infos[0].scope, PluginScope::Global);
    }

    #[test]
    fn expand_strips_frontmatter_and_replaces_positionals() {
        let global = tempfile::tempdir().unwrap();
        install_plugin(global.path(), "a", Some("alpha"));
        write_command(
            &plugin_root(global.path(), &source_id("a")),
            "hello.md",
            "---\ndescription: say hi\n---\nHello $1 and $2!\n",
        );
        let expanded = expand_plugin_command_in(global.path(), None, "/alpha:hello world all")
            .unwrap()
            .expect("expands");
        assert_eq!(expanded, "Hello world and all!");
        // The bare form resolves too.
        assert_eq!(
            expand_plugin_command_in(global.path(), None, "/hello world all")
                .unwrap()
                .as_deref(),
            Some("Hello world and all!")
        );
    }

    #[test]
    fn expand_replaces_full_arguments_placeholder() {
        let global = tempfile::tempdir().unwrap();
        install_plugin(global.path(), "a", Some("alpha"));
        write_command(
            &plugin_root(global.path(), &source_id("a")),
            "echo.md",
            "$ARGUMENTS now",
        );
        assert_eq!(
            expand_plugin_command_in(global.path(), None, "/alpha:echo a b c")
                .unwrap()
                .as_deref(),
            Some("a b c now")
        );
    }

    #[test]
    fn expand_appends_unreferenced_arguments() {
        let global = tempfile::tempdir().unwrap();
        install_plugin(global.path(), "a", Some("alpha"));
        write_command(
            &plugin_root(global.path(), &source_id("a")),
            "echo.md",
            "Body text",
        );
        assert_eq!(
            expand_plugin_command_in(global.path(), None, "/alpha:echo extra words")
                .unwrap()
                .as_deref(),
            Some("Body text\n\nextra words")
        );
        // No arguments → body untouched.
        assert_eq!(
            expand_plugin_command_in(global.path(), None, "/alpha:echo")
                .unwrap()
                .as_deref(),
            Some("Body text")
        );
    }

    #[test]
    fn builtin_commands_are_never_shadowed() {
        let global = tempfile::tempdir().unwrap();
        install_plugin(global.path(), "a", Some("alpha"));
        write_command(
            &plugin_root(global.path(), &source_id("a")),
            "clear.md",
            "stolen",
        );
        // The parser owns /clear, so expansion must bail to it...
        assert!(expand_plugin_command_in(global.path(), None, "/clear")
            .unwrap()
            .is_none());
        // ...and /clear never gets the bare form either, only the namespaced one.
        let infos = list_plugin_commands_in(global.path(), None).unwrap();
        assert!(infos.iter().all(|i| i.bare_invocation.is_none()));
        assert!(infos.iter().any(|i| i.invocation == "/alpha:clear"));
        assert_eq!(
            expand_plugin_command_in(global.path(), None, "/alpha:clear")
                .unwrap()
                .as_deref(),
            Some("stolen")
        );
    }

    #[test]
    fn duplicate_bare_names_are_only_namespaced() {
        let global = tempfile::tempdir().unwrap();
        install_plugin(global.path(), "a", Some("alpha"));
        install_plugin(global.path(), "b", Some("beta"));
        write_command(
            &plugin_root(global.path(), &source_id("a")),
            "deploy.md",
            "A",
        );
        write_command(
            &plugin_root(global.path(), &source_id("b")),
            "deploy.md",
            "B",
        );
        let infos = list_plugin_commands_in(global.path(), None).unwrap();
        assert_eq!(infos.len(), 2);
        assert!(infos.iter().all(|i| i.bare_invocation.is_none()));
        let namespaced: Vec<&str> = infos.iter().map(|i| i.invocation.as_str()).collect();
        assert_eq!(namespaced, ["/alpha:deploy", "/beta:deploy"]);
        // Unknown bare form falls through (caller keeps its own error path).
        assert!(expand_plugin_command_in(global.path(), None, "/deploy")
            .unwrap()
            .is_none());
    }

    #[test]
    fn session_plugin_shadows_global_same_source_id() {
        let global = tempfile::tempdir().unwrap();
        let session = tempfile::tempdir().unwrap();
        install_plugin(global.path(), "a", Some("alpha"));
        write_command(
            &plugin_root(global.path(), &source_id("a")),
            "hello.md",
            "global body",
        );
        install_plugin(session.path(), "a", Some("alpha"));
        write_command(
            &plugin_root(session.path(), &source_id("a")),
            "hello.md",
            "session body",
        );
        let infos = list_plugin_commands_in(global.path(), Some(session.path())).unwrap();
        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0].scope, PluginScope::Session);
        assert_eq!(
            expand_plugin_command_in(global.path(), Some(session.path()), "/alpha:hello")
                .unwrap()
                .as_deref(),
            Some("session body")
        );
        // Without the session store the global body is used.
        assert_eq!(
            expand_plugin_command_in(global.path(), None, "/alpha:hello")
                .unwrap()
                .as_deref(),
            Some("global body")
        );
    }

    #[test]
    fn nested_commands_namespace_with_colons() {
        let global = tempfile::tempdir().unwrap();
        install_plugin(global.path(), "a", Some("alpha"));
        write_command(
            &plugin_root(global.path(), &source_id("a")),
            "git\\commit.md",
            "commit body",
        );
        let infos = list_plugin_commands_in(global.path(), None).unwrap();
        assert_eq!(infos[0].command, "git:commit");
        assert_eq!(infos[0].invocation, "/alpha:git:commit");
        assert_eq!(infos[0].bare_invocation.as_deref(), Some("/git:commit"));
        assert_eq!(
            expand_plugin_command_in(global.path(), None, "/git:commit")
                .unwrap()
                .as_deref(),
            Some("commit body")
        );
    }

    #[test]
    fn preview_scan_reports_command_metadata() {
        let dir = tempfile::tempdir().unwrap();
        write_command(
            dir.path(),
            "hello.md",
            "---\ndescription: greet\nargument-hint: <who>\n---\nbody\n",
        );
        write_command(dir.path(), "plain.md", "no frontmatter");
        let previews = scan_plugin_commands(dir.path());
        assert_eq!(previews.len(), 2);
        assert_eq!(previews[0].name, "hello");
        assert_eq!(previews[0].description.as_deref(), Some("greet"));
        assert_eq!(previews[0].argument_hint.as_deref(), Some("<who>"));
        assert_eq!(previews[1].name, "plain");
        assert_eq!(previews[1].description, None);
    }

    #[test]
    fn expand_arguments_helper() {
        assert_eq!(expand_arguments("$ARGUMENTS now", "a b c"), "a b c now");
        assert_eq!(expand_arguments("$1/$2", "x y z"), "x/y");
        assert_eq!(expand_arguments("no placeholder", ""), "no placeholder");
    }
}
