pub mod manager;
pub mod types;
pub mod workspace_override;

pub use manager::*;
pub use types::*;
pub use workspace_override::*;

use log::error;
use std::path::PathBuf;
use std::sync::OnceLock;

static SESSION_MANAGER: OnceLock<SessionManager> = OnceLock::new();

pub fn get_session_manager() -> Result<&'static SessionManager, String> {
    SESSION_MANAGER.get_or_init(|| {
        SessionManager::new().unwrap_or_else(|e| {
            error!("Failed to initialize SessionManager: {e}");
            // Create fallback session manager with temp directory
            let temp_base = std::env::temp_dir().join("com.fritzprix.libragent");

            // We need to use the constructor that initializes directory service
            match SessionManager::new_with_base_dir(temp_base) {
                Ok(manager) => manager,
                Err(err) => {
                    // Critical failure: if we can't create temp directories, the application
                    // cannot function. Panicking is preferable to returning a broken instance.
                    panic!("Critical error initializing SessionManager fallback: {err}");
                }
            }
        })
    });
    Ok(SESSION_MANAGER.get().unwrap())
}

pub fn teamwork_artifact_dir_for_session(
    session_manager: &SessionManager,
    session_id: &str,
) -> PathBuf {
    session_manager
        .get_directory_service()
        .get_teamwork_artifact_dir_unverified(session_id)
}

pub const TEAMWORK_PARENT_CHAIN_LIMIT: usize = 64;

/// Extracts the scoped relative path if the candidate matches `@teamwork` or `.libragent/teamwork`.
pub fn extract_teamwork_alias_relative_path(path_str: &str) -> Option<&str> {
    let trimmed = path_str.trim();
    let stripped = trimmed
        .strip_prefix("./")
        .or_else(|| trimmed.strip_prefix(".\\"))
        .or_else(|| trimmed.strip_prefix("/workspace/"))
        .or_else(|| trimmed.strip_prefix("\\workspace\\"))
        .or_else(|| {
            if trimmed.starts_with("/@teamwork")
                || trimmed.starts_with("/.libragent/teamwork")
                || trimmed.starts_with("\\@teamwork")
                || trimmed.starts_with("\\.libragent\\teamwork")
            {
                Some(&trimmed[1..])
            } else {
                None
            }
        })
        .unwrap_or(trimmed);

    if stripped == "@teamwork"
        || stripped == ".libragent/teamwork"
        || stripped == ".libragent\\teamwork"
    {
        return Some(".");
    }

    stripped
        .strip_prefix("@teamwork/")
        .or_else(|| stripped.strip_prefix("@teamwork\\"))
        .or_else(|| stripped.strip_prefix(".libragent/teamwork/"))
        .or_else(|| stripped.strip_prefix(".libragent\\teamwork\\"))
        .map(|suffix| {
            let s = suffix.trim();
            if s.is_empty() {
                "."
            } else {
                s
            }
        })
}

/// Resolves the governing root session ID for teamwork artifacts by following
/// org root or parent session chains up to TEAMWORK_PARENT_CHAIN_LIMIT hops.
pub async fn resolve_teamwork_root_session_id(
    session_repo: Option<&dyn crate::repositories::SessionRepository>,
    session_id: &str,
) -> Result<String, String> {
    let Some(repo) = session_repo else {
        return Ok(session_id.to_string());
    };

    let mut current = match repo.get_session(session_id).await {
        Ok(Some(session)) => session,
        Ok(None) => return Ok(session_id.to_string()),
        Err(e) => {
            log::warn!(
                "Could not load session {session_id} from repository: {e}; using session_id as root"
            );
            return Ok(session_id.to_string());
        }
    };

    if let Some(org_root_session_id) = current.org_root_session_id.clone() {
        return Ok(org_root_session_id);
    }

    for _ in 0..TEAMWORK_PARENT_CHAIN_LIMIT {
        let Some(parent_session_id) = current.parent_session_id.clone() else {
            return Ok(current.id);
        };

        current = match repo
            .get_session(&parent_session_id)
            .await
            .map_err(|e| format!("Failed to load parent session metadata: {e}"))?
        {
            Some(session) => session,
            None => return Ok(parent_session_id),
        };

        if let Some(org_root_session_id) = current.org_root_session_id.clone() {
            return Ok(org_root_session_id);
        }
    }

    log::warn!(
        "Teamwork root search for session {session_id} hit parent chain limit ({TEAMWORK_PARENT_CHAIN_LIMIT}); falling back to current session {}",
        current.id
    );
    Ok(current.id)
}

/// Resolves the teamwork artifact directory for a given session by looking up
/// the root session ID and computing its artifact dir path.
pub async fn resolve_teamwork_artifact_dir(
    session_manager: &SessionManager,
    session_id: &str,
) -> Result<PathBuf, String> {
    let repo_holder = crate::state::try_get_session_repository();
    let repo_ref = repo_holder.map(|r| r as &dyn crate::repositories::SessionRepository);
    let root_session_id = resolve_teamwork_root_session_id(repo_ref, session_id).await?;
    Ok(teamwork_artifact_dir_for_session(
        session_manager,
        &root_session_id,
    ))
}

/// Prepare the app-local teamwork artifact directory for a governing/root session.
///
/// This path is for durable teamwork scaffolding and coordination metadata only.
/// It is intentionally separate from the session's effective workspace so org roots
/// and children keep sharing the normal parent/override workspace semantics.
pub async fn prepare_teamwork_artifact_dir_for_session(
    session_manager: &SessionManager,
    session_id: &str,
) -> Result<PathBuf, String> {
    session_manager
        .get_directory_service()
        .create_teamwork_artifact_dir(session_id)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    #[test]
    fn test_session_isolation() {
        // Setup temp dir for base
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let base_path = temp_dir.path().to_path_buf();

        // Initialize SessionManager manually
        let session_manager =
            SessionManager::new_with_base_dir(base_path.clone()).expect("Failed to init");

        // Get two distinct session paths
        let session_id_a = "session_a";
        let session_id_b = "session_b";
        let path_a = session_manager.get_session_workspace_dir_by_id(session_id_a);
        let path_b = session_manager.get_session_workspace_dir_by_id(session_id_b);

        // Verify paths are different and correctly structured
        assert_ne!(path_a, path_b);
        // Use path-aware checks that work across Windows/Unix
        assert!(path_a.ends_with(Path::new("workspaces").join("session_a")));
        assert!(path_b.ends_with(Path::new("workspaces").join("session_b")));

        // Create a file in Session A
        let file_a = path_a.join("test.txt");
        // Ensure directory exists (SessionManager lazy creation might need explicit trigger in test environment if async creation wasn't awaited)
        // However, get_session_workspace_dir_by_id calls get_workspace_dir which we modified to create if missing.
        // Let's verify dir existence first
        assert!(path_a.exists(), "Session A path should exist");

        fs::write(&file_a, "Hello A").expect("Failed to write file A");

        // Verify it exists in A but NOT in B path
        assert!(file_a.exists());
        let file_b_location = path_b.join("test.txt");
        assert!(!file_b_location.exists());

        // Create file in Session B
        let file_b = path_b.join("other.txt");
        assert!(path_b.exists(), "Session B path should exist");
        fs::write(&file_b, "Hello B").expect("Failed to write file B");

        assert!(file_b.exists());
        let file_a_location = path_a.join("other.txt");
        assert!(!file_a_location.exists());
    }

    #[test]
    fn test_default_fallback_session_path() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let base_path = temp_dir.path().to_path_buf();

        let session_manager =
            SessionManager::new_with_base_dir(base_path.clone()).expect("Failed to init");

        // If something requests "default" explicitly
        let path_default = session_manager.get_session_workspace_dir_by_id("default");
        assert!(path_default
            .to_string_lossy()
            .contains("workspaces/default"));
        assert!(path_default.exists());
    }

    #[tokio::test]
    async fn test_session_workspace_override() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let base_path = temp_dir.path().to_path_buf();
        let session_manager =
            SessionManager::new_with_base_dir(base_path.clone()).expect("Failed to init");

        let session_id = "override_test_session";
        let override_dir = temp_dir.path().join("my_custom_workspace");
        fs::create_dir(&override_dir).unwrap();

        // Register override
        session_manager
            .register_session_override(session_id, override_dir.clone())
            .await
            .expect("Failed to register override");

        // Get path, should be the override
        let path = session_manager.get_session_workspace_dir_by_id(session_id);
        assert_eq!(path, override_dir);

        // Remove override
        session_manager
            .remove_workspace_override(session_id)
            .await
            .expect("Failed to remove override");

        // Get path again, should fallback to default (which should have been created during register)
        let path_after = session_manager.get_session_workspace_dir_by_id(session_id);
        assert_ne!(path_after, override_dir);
        assert!(path_after.ends_with(Path::new("workspaces").join(session_id)));
        assert!(
            path_after.exists(),
            "Default workspace should exist after fallback"
        );
    }
}
