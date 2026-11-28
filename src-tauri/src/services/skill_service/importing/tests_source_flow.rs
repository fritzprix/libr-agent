//! #2928 gatekeeper G1.7: a generic git source (`file://` repository) and a
//! local directory must run through the real preview and install flow,
//! including the hook and MCP approval data the record carries.

use super::super::contracts::SKILL_FILE_NAME;
use super::plugin_install::{
    build_hook_preview_entries, build_mcp_preview_entries, install_plugin_source, plugin_source_id,
    try_load_plugin_hooks,
};
use super::preview::build_import_preview;
use super::source::prepare_git_skill_import;
use super::tests_hook_preview::{write_manifest, write_valid_skill};
use std::fs;
use std::path::Path;

fn git_available() -> bool {
    std::process::Command::new("git")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

/// Write the plugin layout (manifest, skill, hooks, MCP) into `root`.
fn write_plugin_layout(root: &Path, skill_name: &str) {
    write_manifest(root, r#"{ "name": "flow-plugin", "version": "1.0.0" }"#);
    write_valid_skill(&root.join("skills").join(skill_name), skill_name);
    let hooks_dir = root.join("hooks");
    fs::create_dir_all(&hooks_dir).unwrap();
    fs::write(
        hooks_dir.join("hooks.json"),
        r#"{"hooks":{"PreToolUse":[{"matcher":"workspace__runShell","hooks":[
            {"type":"command","command":"echo hook"}
        ]}]}}"#,
    )
    .unwrap();
    fs::write(
        root.join(".mcp.json"),
        r#"{"mcpServers":{"flow":{"command":"uvx","args":["flow-server"]}}}"#,
    )
    .unwrap();
}

/// A committed git repository carrying the plugin layout; with
/// `feature_skill` it also holds a `feature` branch whose plugin replaces
/// the main skill with `<feature_skill>`.
fn init_plugin_repo(path: &Path, feature_skill: Option<&str>) {
    write_plugin_layout(path, "main-skill");
    let git = |args: &[&str]| {
        let output = std::process::Command::new("git")
            .current_dir(path)
            .args(args)
            .output()
            .unwrap();
        assert!(output.status.success(), "git {args:?}: {output:?}");
    };
    git(&["init", "--initial-branch=main"]);
    for args in [
        ["config", "user.email", "t@t"].as_slice(),
        ["config", "user.name", "t"].as_slice(),
        ["add", "-A"].as_slice(),
        ["commit", "-m", "init"].as_slice(),
    ] {
        git(args);
    }

    if let Some(skill_name) = feature_skill {
        for args in [
            ["checkout", "-b", "feature"].as_slice(),
            ["rm", "-r", "skills"].as_slice(),
        ] {
            git(args);
        }
        fs::remove_dir_all(path.join("skills")).ok();
        write_valid_skill(&path.join("skills").join(skill_name), skill_name);
        for args in [
            ["add", "-A"].as_slice(),
            ["commit", "-m", "feature"].as_slice(),
        ] {
            git(args);
        }
        git(&["checkout", "main"]);
    }
}

fn file_url(path: &Path) -> String {
    url::Url::from_file_path(path)
        .expect("absolute temp path")
        .to_string()
}

async fn prepare_repo_import(
    repo: &Path,
    temp_root: &Path,
    git_ref: Option<&str>,
) -> super::super::contracts::PreparedSkillImport {
    prepare_git_skill_import(
        file_url(repo),
        repo.to_string_lossy().to_string(),
        git_ref.map(str::to_string),
        Some(temp_root),
    )
    .await
    .unwrap()
}

/// G1.7: a `file://` git source runs through the real preview and a full
/// install; the persisted record carries the approved hooks and MCP servers,
/// and the clone's `.git` never reaches the installed copy.
#[tokio::test]
async fn git_source_previews_and_installs_with_hook_and_mcp_approval() {
    if !git_available() {
        return;
    }
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");
    init_plugin_repo(&repo, None);
    let temp_root = temp.path().join("temp-root");
    fs::create_dir_all(&temp_root).unwrap();

    let mut prepared = prepare_repo_import(&repo, &temp_root, None).await;
    // The clone's `.git` is removed before scanning, so nothing but plugin
    // content is staged.
    assert!(!prepared.scan_root.join(".git").exists());
    assert_eq!(prepared.discovered_skills.len(), 1);

    let target = tempfile::tempdir().unwrap();
    let system = tempfile::tempdir().unwrap();
    let preview = build_import_preview(
        &mut prepared,
        target.path(),
        "global",
        "user",
        system.path(),
        &[],
        None,
    )
    .await
    .unwrap();
    assert_eq!(preview.discovered_skills[0].name, "main-skill");
    let hooks = try_load_plugin_hooks(&prepared.scan_root, None)
        .unwrap()
        .expect("the cloned plugin carries hooks");
    let hook_entries = build_hook_preview_entries(&hooks);
    assert_eq!(hook_entries.len(), 1);
    assert_eq!(hook_entries[0].command, "echo hook");
    let mcp_servers =
        super::super::plugin_mcp::collect_plugin_mcp_servers(&prepared.scan_root, None).unwrap();
    let mcp_entries = build_mcp_preview_entries(&mcp_servers);
    assert_eq!(mcp_entries.len(), 1);
    assert_eq!(mcp_entries[0].name, "flow");

    // Install with the one-time approval the dialog represents.
    let plugins = tempfile::tempdir().unwrap();
    let source_key = prepared.source_key.clone();
    install_plugin_source(
        &prepared.scan_root,
        &source_key,
        &["main-skill".to_string()],
        true,
        plugins.path(),
    )
    .unwrap();
    let record = crate::services::hooks::registry::get_source_in(
        plugins.path(),
        &plugin_source_id(&source_key),
    )
    .unwrap();
    assert!(record.manifest.hooks_approved, "hooks were approved");
    assert!(record.manifest.mcp_approved, "MCP was approved");
    assert_eq!(record.manifest.mcp_servers.len(), 1);
    assert_eq!(record.manifest.hook_files, ["hooks/hooks.json".to_string()]);
    assert_eq!(record.manifest.skill_names, ["main-skill".to_string()]);
    assert_eq!(record.manifest.source_url, source_key);

    // The installed copy carries no repository state.
    let installed = plugins.path().join(plugin_source_id(&source_key));
    assert!(!installed.join(".git").exists());
    assert!(installed
        .join("skills")
        .join("main-skill")
        .join(SKILL_FILE_NAME)
        .is_file());
}

/// G1.7: the same flow without approval records the declarations but marks
/// nothing approved, so nothing is started.
#[tokio::test]
async fn git_source_install_without_approval_records_but_does_not_approve() {
    if !git_available() {
        return;
    }
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");
    init_plugin_repo(&repo, None);
    let temp_root = temp.path().join("temp-root");
    fs::create_dir_all(&temp_root).unwrap();

    let prepared = prepare_repo_import(&repo, &temp_root, None).await;
    let plugins = tempfile::tempdir().unwrap();
    let source_key = prepared.source_key.clone();
    install_plugin_source(
        &prepared.scan_root,
        &source_key,
        &["main-skill".to_string()],
        false,
        plugins.path(),
    )
    .unwrap();
    let record = crate::services::hooks::registry::get_source_in(
        plugins.path(),
        &plugin_source_id(&source_key),
    )
    .unwrap();
    assert!(!record.manifest.hooks_approved);
    assert!(!record.manifest.mcp_approved);
    assert_eq!(record.manifest.mcp_servers.len(), 1);
}

/// G1.6 end to end: `?ref=` on a `file://` source selects the branch whose
/// skills get installed, instead of silently taking the default branch.
#[tokio::test]
async fn git_source_ref_selects_the_branch_through_the_flow() {
    if !git_available() {
        return;
    }
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");
    init_plugin_repo(&repo, Some("feature-skill"));
    let temp_root = temp.path().join("temp-root");
    fs::create_dir_all(&temp_root).unwrap();

    let mut prepared = prepare_repo_import(&repo, &temp_root, Some("feature")).await;
    let target = tempfile::tempdir().unwrap();
    let system = tempfile::tempdir().unwrap();
    let preview = build_import_preview(
        &mut prepared,
        target.path(),
        "global",
        "user",
        system.path(),
        &[],
        None,
    )
    .await
    .unwrap();
    assert_eq!(preview.discovered_skills.len(), 1);
    assert_eq!(preview.discovered_skills[0].name, "feature-skill");
}

/// G1.1: a symlink inside a fetched repository must never become a symlink
/// in the staged tree, and the linked host file's content must not arrive
/// there. `clone` runs with `core.symlinks=false`, so git checks the link
/// out as a plain file holding its target text; any symlink that still
/// appears is rejected outright by the fetched-tree check.
#[cfg(unix)]
#[tokio::test]
async fn git_source_symlink_never_reaches_the_staged_tree() {
    if !git_available() {
        return;
    }
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");
    init_plugin_repo(&repo, None);
    // A link to a file whose content the test can recognize if it leaked.
    let secret = temp.path().join("secret.txt");
    let secret_text = "SECRET-HOST-CONTENT-4821";
    fs::write(&secret, secret_text).unwrap();
    std::os::unix::fs::symlink(&secret, repo.join("skills").join("leak")).unwrap();
    for args in [
        ["add", "-A"].as_slice(),
        ["commit", "-m", "symlink"].as_slice(),
    ] {
        let output = std::process::Command::new("git")
            .current_dir(&repo)
            .args(args)
            .output()
            .unwrap();
        assert!(output.status.success(), "git {args:?}: {output:?}");
    }
    let temp_root = temp.path().join("temp-root");
    fs::create_dir_all(&temp_root).unwrap();

    let staged = match prepare_git_skill_import(
        file_url(&repo),
        repo.to_string_lossy().to_string(),
        None,
        Some(&temp_root),
    )
    .await
    {
        Ok(staged) => staged,
        // The fetched-tree check rejecting the source is also a pass.
        Err(error) => {
            assert!(error.contains("symlink"), "{error}");
            return;
        }
    };
    for entry in walkdir::WalkDir::new(&staged.scan_root)
        .follow_links(false)
        .into_iter()
        .filter_map(|entry| entry.ok())
    {
        assert!(
            !entry.file_type().is_symlink(),
            "a staged symlink would escape the plugin directory: {}",
            entry.path().display()
        );
        if entry.file_type().is_file() {
            let content = fs::read_to_string(entry.path()).unwrap_or_default();
            assert!(
                !content.contains(secret_text),
                "the linked host file's content leaked into {}",
                entry.path().display()
            );
        }
    }
}

/// G1.7 (local branch of the router): a plain local directory source runs
/// through preview and install as well.
#[tokio::test]
async fn local_directory_source_previews_and_installs() {
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().join("plugin-dir");
    write_plugin_layout(&dir, "dir-skill");

    let mut prepared =
        super::archive::prepare_local_skill_import_blocking(temp.path(), &dir.to_string_lossy())
            .unwrap();
    let target = tempfile::tempdir().unwrap();
    let system = tempfile::tempdir().unwrap();
    let preview = build_import_preview(
        &mut prepared,
        target.path(),
        "global",
        "user",
        system.path(),
        &[],
        None,
    )
    .await
    .unwrap();
    assert_eq!(preview.discovered_skills[0].name, "dir-skill");

    let plugins = tempfile::tempdir().unwrap();
    let source_key = prepared.source_key.clone();
    install_plugin_source(
        &prepared.scan_root,
        &source_key,
        &["dir-skill".to_string()],
        true,
        plugins.path(),
    )
    .unwrap();
    let record = crate::services::hooks::registry::get_source_in(
        plugins.path(),
        &plugin_source_id(&source_key),
    )
    .unwrap();
    assert!(record.manifest.hooks_approved);
    assert_eq!(record.manifest.skill_names, ["dir-skill".to_string()]);
}
