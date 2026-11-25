use super::contracts::{PluginSourceOverview, PluginUpdateStatus};
use super::directories::get_user_skills_directory;
use super::github::parse_github_repo_url;
use super::importing::remove_skill_names_from_user_skills;
use crate::services::hooks::registry::{
    delete_source, get_source, hook_config_for_record, list_sources, list_sources_in,
    PluginSourceRecord,
};
use std::path::Path;

pub fn list_plugin_source_overviews() -> Result<Vec<PluginSourceOverview>, String> {
    let records = list_sources()?;
    let mut out = records
        .into_iter()
        .map(|record| {
            let hook_events = record
                .manifest
                .hooks
                .as_ref()
                .map(|_| hook_events_for_record(&record))
                .unwrap_or_default();
            PluginSourceOverview {
                id: record.manifest.source_id.clone(),
                source_url: record.manifest.source_url.clone(),
                skill_names: record.manifest.skill_names.clone(),
                hook_events,
                hooks_approved: record.manifest.hooks_approved,
                plugin_name: record.manifest.plugin.as_ref().map(|p| p.name.clone()),
                plugin_version: record
                    .manifest
                    .plugin
                    .as_ref()
                    .and_then(|p| p.version.clone()),
                plugin_description: record
                    .manifest
                    .plugin
                    .as_ref()
                    .and_then(|p| p.description.clone()),
                enabled: record.manifest.enabled,
                installed_commit: record.manifest.installed_commit.clone(),
            }
        })
        .collect::<Vec<_>>();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}

/// Hook events a source declares, loaded through the same record-aware
/// loader as dispatch so manifest-declared hook files are listed too
/// (gatekeeper G2.2).
pub(super) fn hook_events_for_record(record: &PluginSourceRecord) -> Vec<String> {
    hook_config_for_record(record)
        .ok()
        .map(|cfg| {
            let mut events: Vec<String> = cfg
                .events
                .keys()
                .map(|event| event.as_str().to_string())
                .collect();
            events.sort();
            events.dedup();
            events
        })
        .unwrap_or_default()
}

pub async fn delete_plugin_source(source_id: String) -> Result<(), String> {
    let record = get_source(&source_id)?;
    let user_dir = get_user_skills_directory()?;
    remove_skill_names_from_user_skills(&user_dir, &record.manifest.skill_names, &source_id)?;
    // #2924: removing the plugin also stops and removes the global MCP
    // servers it owned. Session-scoped servers never entered the global
    // store; their session plugin dir (and proxy) is torn down separately.
    if !record.manifest.mcp_servers.is_empty() {
        super::plugin_mcp::remove_global_plugin_mcp_servers(&source_id).await?;
    }
    // #2923 (D2): the plugin's `agents/` assistant rows go with it, and only
    // with it. The marker sweep needs no agent list, so it runs even when
    // the installed tree no longer contains the agents directory.
    super::plugin_agents::remove_global_plugin_agents(&source_id).await?;
    // #2926: uninstall is the only thing that removes `${CLAUDE_PLUGIN_DATA}`
    // (a plugin update keeps it) and the plugin's stored `userConfig` values:
    // the sidecar next to the store entry and the sensitive values in secure
    // storage.
    remove_global_plugin_data(&source_id)?;
    let store_dir = crate::services::hooks::registry::get_user_plugins_directory()?;
    let saved = super::plugin_options::read_saved_options(&store_dir, &source_id);
    if let Err(error) = super::plugin_options::remove_saved_options(&store_dir, &source_id) {
        log::warn!("plugin {source_id}: could not remove stored options: {error}");
    }
    if let Err(error) =
        super::plugin_options::delete_sensitive_options(&source_id, None, &saved.sensitive_keys)
            .await
    {
        log::warn!("plugin {source_id}: could not delete secure option values: {error}");
    }
    delete_source(&source_id)?;
    // #2812 criterion 4: uninstalling a source must not leave its hook
    // bootstrap in any open session's prompt slot.
    on_plugin_source_uninstalled();
    super::cache::invalidate_skill_scan_cache();
    Ok(())
}

/// Delete a global plugin's `${CLAUDE_PLUGIN_DATA}` directory (#2926). A
/// missing directory is fine; a real failure is returned so the caller can
/// report that uninstall left data behind.
pub fn remove_global_plugin_data(source_id: &str) -> Result<(), String> {
    use crate::services::hooks::registry::get_user_plugin_data_dir;
    let dir = match get_user_plugin_data_dir(source_id) {
        Ok(dir) => dir,
        // No session manager (unit tests): nothing can have been created.
        Err(_) => return Ok(()),
    };
    remove_plugin_data_dir(&dir)
}

/// Remove one plugin data directory, ignoring a missing one.
pub fn remove_plugin_data_dir(dir: &std::path::Path) -> Result<(), String> {
    match std::fs::remove_dir_all(dir) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("remove plugin data dir {}: {error}", dir.display())),
    }
}

/// Drop every session's hook-context slot after a plugin source uninstall.
/// Kept as a named step so criterion-4 tests can exercise the same path
/// `delete_plugin_source` uses without needing a live SessionManager.
pub(crate) fn on_plugin_source_uninstalled() {
    crate::services::hooks::clear_all_hook_contexts();
}

/// Source ids of global plugins that are installed but disabled (#2927).
/// Their plugin-owned MCP servers stay in the DB and are skipped at load, so
/// enabling again needs no re-creation.
pub fn disabled_global_plugin_source_ids() -> std::collections::HashSet<String> {
    match crate::services::hooks::registry::get_user_plugins_directory() {
        Ok(base) => disabled_source_ids_in(&base),
        Err(_) => std::collections::HashSet::new(),
    }
}

/// The store scan behind [`disabled_global_plugin_source_ids`], split out so
/// the MCP loader can be driven in a test against a temporary store instead
/// of the real app data directory (gatekeeper G1.2).
pub(crate) fn disabled_source_ids_in(base: &Path) -> std::collections::HashSet<String> {
    list_sources_in(base)
        .unwrap_or_default()
        .into_iter()
        .filter(|r| !r.manifest.enabled)
        .map(|r| r.manifest.source_id)
        .collect()
}

/// Turn one global plugin on or off (#2927). The plugin keeps its files,
/// approval records, data directory, and MCP server rows; loaders skip it
/// while it is disabled. An open session picks the change up when it opens
/// again or on `/reload` (owner decision D5).
pub fn set_plugin_enabled(source_id: &str, enabled: bool) -> Result<(), String> {
    let base = crate::services::hooks::registry::get_user_plugins_directory()?;
    crate::services::hooks::registry::set_source_enabled_in(&base, source_id, enabled)?;
    // Hooks stop now rather than at the next session open: the bootstrap text
    // a disabled plugin wrote is stale in every open session's prompt slot.
    on_plugin_source_uninstalled();
    Ok(())
}

/// Whether the GitHub commits API answers "is there a newer commit?" for one
/// installed plugin (#2927 D6). A pinned branch/tag is compared with its own
/// ref; a plain URL with the commit recorded at install is compared with the
/// repository default branch.
pub(crate) fn github_head_commits_url(spec: &super::contracts::GitHubRepoSpec) -> String {
    let base = format!("https://api.github.com/repos/{}/{}", spec.owner, spec.repo);
    match spec.branch.as_deref() {
        Some(branch) => format!("{base}/commits/{branch}"),
        None => format!("{base}/commits?per_page=1"),
    }
}

/// First commit sha of a GitHub `/commits` response: an array of commits for
/// the list form, a single commit object for the `/{ref}` form (#2927 D6).
pub(crate) fn head_sha_from_commits_json(raw: &serde_json::Value) -> Option<String> {
    let entry = match raw.get(0) {
        Some(first) => first,
        None => raw,
    };
    entry
        .get("sha")
        .and_then(|sha| sha.as_str())
        .map(|sha| sha.to_ascii_lowercase())
}

/// An update is available only when a recorded install commit differs from
/// the remote head (#2927 D6). Without a recorded commit there is nothing
/// to compare, so the indicator stays off rather than guessing.
pub(crate) fn update_available(installed_commit: Option<&str>, remote_head: Option<&str>) -> bool {
    match (installed_commit, remote_head) {
        (Some(installed), Some(remote)) => !installed.eq_ignore_ascii_case(remote),
        _ => false,
    }
}

async fn fetch_remote_head(url: &str) -> Result<Option<String>, String> {
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(
            super::contracts::GITHUB_DOWNLOAD_CONNECT_TIMEOUT_SECS,
        ))
        .timeout(std::time::Duration::from_secs(
            super::contracts::GITHUB_DOWNLOAD_TIMEOUT_SECS,
        ))
        .build()
        .map_err(|error| error.to_string())?;
    let response = client
        .get(url)
        .header(reqwest::header::USER_AGENT, "libragent-plugin-update-check")
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!("GitHub commits API: HTTP {}", response.status()));
    }
    let raw: serde_json::Value = response.json().await.map_err(|error| error.to_string())?;
    Ok(head_sha_from_commits_json(&raw))
}

/// Check every global plugin for a newer source (#2927 D6 indicator). Only
/// plugins with a recorded `installed_commit` are queried; a remote failure
/// reports `update_available: false` — the indicator must never claim an
/// update it cannot show.
pub async fn check_plugin_source_updates() -> Result<Vec<PluginUpdateStatus>, String> {
    let records = list_sources()?;
    let mut out = Vec::with_capacity(records.len());
    for record in records {
        let source_id = record.manifest.source_id.clone();
        let installed = record.manifest.installed_commit.clone();
        let spec = parse_github_repo_url(&record.manifest.source_url).ok();
        let remote = match (&installed, spec) {
            (Some(_), Some(spec)) => {
                match fetch_remote_head(&github_head_commits_url(&spec)).await {
                    Ok(remote) => remote,
                    Err(error) => {
                        log::warn!("plugin update check for {source_id} failed: {error}");
                        None
                    }
                }
            }
            _ => None,
        };
        out.push(PluginUpdateStatus {
            source_id,
            update_available: update_available(installed.as_deref(), remote.as_deref()),
        });
    }
    Ok(out)
}

/// Skill names that only disabled plugins provide for one session (#2927).
///
/// A disabled plugin keeps its files, so its skills still exist on disk; the
/// skills prompt must drop them by name. A session install shadows a global
/// install with the same `source_id` (same rule as hooks), so a disabled
/// global copy does not hide an enabled session copy.
pub fn disabled_plugin_skill_names(session_id: &str) -> std::collections::HashSet<String> {
    let session_records =
        super::session_plugins::load_session_plugins(session_id).unwrap_or_default();
    let global_records = match crate::services::hooks::registry::get_user_plugins_directory() {
        Ok(base) => list_sources_in(&base).unwrap_or_default(),
        Err(_) => Vec::new(),
    };
    disabled_skill_names_in(&session_records, &global_records)
}

/// The name filter behind [`disabled_plugin_skill_names`], split out so the
/// shadowing rule is testable without a real app data directory.
fn disabled_skill_names_in(
    session_records: &[PluginSourceRecord],
    global_records: &[PluginSourceRecord],
) -> std::collections::HashSet<String> {
    use std::collections::HashSet;

    let mut names = HashSet::new();
    for record in session_records.iter().filter(|r| !r.manifest.enabled) {
        names.extend(record.manifest.skill_names.iter().cloned());
    }
    let shadowed: HashSet<String> = session_records
        .iter()
        .map(|r| r.manifest.source_id.clone())
        .collect();
    for record in global_records.iter().filter(|r| !r.manifest.enabled) {
        if shadowed.contains(&record.manifest.source_id) {
            continue;
        }
        names.extend(record.manifest.skill_names.iter().cloned());
    }
    names
}

#[cfg(test)]
mod tests {
    use super::{disabled_skill_names_in, on_plugin_source_uninstalled};
    use crate::services::hooks::registry::{PluginSourceManifest, PluginSourceRecord};
    use crate::services::hooks::{clear_hook_context, get_hook_context, set_hook_context};
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    fn record(source_id: &str, skills: &[&str], enabled: bool) -> PluginSourceRecord {
        PluginSourceRecord {
            manifest: PluginSourceManifest {
                source_url: format!("https://example.com/{source_id}"),
                source_id: source_id.to_string(),
                skill_names: skills.iter().map(|s| s.to_string()).collect(),
                hooks: None,
                hooks_approved: false,
                approval_hashes: BTreeMap::new(),
                created_at: 0,
                plugin: None,
                hook_files: vec![],
                mcp_servers: vec![],
                mcp_approved: false,
                enabled,
                installed_commit: None,
            },
            root_dir: PathBuf::from("/nonexistent"),
        }
    }

    /// #2927: a disabled plugin hides its skills from the prompt, in either
    /// scope, while an enabled copy of the same source stays visible.
    #[test]
    fn disabled_plugins_hide_their_skills() {
        let session = vec![
            record("plugin-aaaa000000000000", &["session-off"], false),
            record("plugin-bbbb000000000000", &["session-on"], true),
        ];
        let global = vec![
            // Shadowed by the session entry above: its skills stay hidden
            // even though the session copy is disabled.
            record("plugin-aaaa000000000000", &["global-shadowed"], false),
            record("plugin-cccc000000000000", &["global-off"], false),
            record("plugin-dddd000000000000", &["global-on"], true),
        ];

        let hidden = disabled_skill_names_in(&session, &global);
        let mut names: Vec<&str> = hidden.iter().map(String::as_str).collect();
        names.sort_unstable();
        assert_eq!(names, vec!["global-off", "session-off"]);
    }

    #[test]
    fn uninstall_purge_clears_hook_contexts() {
        let sid = "test-plugin-uninstall-purge";
        clear_hook_context(sid);
        set_hook_context(sid, "stale bootstrap".to_string());
        on_plugin_source_uninstalled();
        assert!(
            get_hook_context(sid).is_none(),
            "delete_plugin_source must clear hook contexts via on_plugin_source_uninstalled"
        );
    }

    // #2927 D6: the update-check decision is pure (URL shape, response
    // parsing, comparison) so the indicator is testable without network.
    #[test]
    fn update_check_urls_and_comparison() {
        use super::super::contracts::GitHubRepoSpec;
        use super::{github_head_commits_url, head_sha_from_commits_json, update_available};

        let plain = GitHubRepoSpec {
            owner: "obra".into(),
            repo: "superpowers".into(),
            branch: None,
            subpath: None,
        };
        let pinned = GitHubRepoSpec {
            branch: Some("dev/1.0".into()),
            ..plain.clone()
        };
        assert_eq!(
            github_head_commits_url(&plain),
            "https://api.github.com/repos/obra/superpowers/commits?per_page=1"
        );
        assert_eq!(
            github_head_commits_url(&pinned),
            "https://api.github.com/repos/obra/superpowers/commits/dev/1.0"
        );

        let array = serde_json::json!([{"sha": "ABCDEF0123456789"}]);
        assert_eq!(
            head_sha_from_commits_json(&array).as_deref(),
            Some("abcdef0123456789"),
            "the list form carries an array; the sha is lower-cased for comparison"
        );
        let single = serde_json::json!({ "sha": "deadbeef" });
        assert_eq!(
            head_sha_from_commits_json(&single).as_deref(),
            Some("deadbeef"),
            "the ref form returns a single commit object"
        );
        assert_eq!(
            head_sha_from_commits_json(&serde_json::json!({ "message": "Not Found" })),
            None
        );
        assert_eq!(head_sha_from_commits_json(&serde_json::json!([])), None);

        assert!(update_available(Some("aaa"), Some("bbb")));
        assert!(
            !update_available(Some("aaa"), Some("AAA")),
            "shas compare case-insensitively"
        );
        assert!(!update_available(Some("aaa"), Some("aaa")));
        assert!(
            !update_available(None, Some("aaa")),
            "no recorded commit means nothing to compare: never claim an update"
        );
        assert!(!update_available(Some("aaa"), None));
    }
}
