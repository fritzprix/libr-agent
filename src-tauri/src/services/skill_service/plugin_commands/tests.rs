//! Slash-command behaviour, moved here by the #3043 split.

use super::discovery::discover_command_files;
use super::expansion::{expand_named_in_dirs, substitute_arguments};
use super::listing::list_in_dirs;
use super::*;
use crate::services::hooks::registry::{
    list_session_store_sources, save_source_in, upsert_session_store_entry, PluginSourceManifest,
    PluginSourceRecord,
};
use crate::services::skill_service::session_plugins::PluginScope;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

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
        inline_hooks: vec![],
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

const HELLO: &str = "---\ndescription: Say hello\nargument-hint: <name>\n---\nHello $ARGUMENTS!\n";

/// #2927 fixture: an installed plugin that the user has switched off.
fn install_disabled(dir: &Path, id: &str, url: &str, files: &[(&str, &str)]) -> PluginSourceRecord {
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
    let expanded = expand_named_in_dirs(
        &global,
        list_session_store_sources(None, &session),
        "/hello world",
    )
    .unwrap();
    assert_eq!(expanded.body, "Hello world!");
    let namespaced = expand_named_in_dirs(
        &global,
        list_session_store_sources(None, &session),
        "/e:hello world",
    )
    .unwrap();
    assert_eq!(namespaced.body, "Hello world!");
    assert!(
        expand_named_in_dirs(&global, list_session_store_sources(None, &session), "/nope")
            .is_none()
    );
    // Built-ins are never expanded by plugin resolution.
    assert!(expand_named_in_dirs(
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
    let (map, body) = split_frontmatter("---\r\ndescription: d\r\n---\r\nBody\r\n---\r\nMore\r\n");
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
