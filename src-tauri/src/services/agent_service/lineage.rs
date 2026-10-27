use crate::agent::types::{CreateSessionRequest, SessionLineageMeta};
use crate::repositories::session_repository::SessionRepository;
use crate::repositories::settings_repository::SettingsRepository;
use crate::repositories::SessionMetadata;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use tokio::sync::RwLock as TokioRwLock;

pub static SESSION_LINEAGE: OnceLock<TokioRwLock<HashMap<String, SessionLineageMeta>>> =
    OnceLock::new();
static PARENT_SPAWN_LOCKS: OnceLock<Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>> =
    OnceLock::new();

pub fn lineage_store() -> &'static TokioRwLock<HashMap<String, SessionLineageMeta>> {
    SESSION_LINEAGE.get_or_init(|| TokioRwLock::new(HashMap::new()))
}

fn parent_spawn_locks() -> &'static Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>> {
    PARENT_SPAWN_LOCKS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub async fn acquire_parent_spawn_guard(
    parent_session_id: Option<&str>,
) -> Option<tokio::sync::OwnedMutexGuard<()>> {
    let parent_session_id = parent_session_id?;
    let lock = {
        let mut locks = parent_spawn_locks()
            .lock()
            .expect("parent spawn lock map should not be poisoned");
        locks
            .entry(parent_session_id.to_string())
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
            .clone()
    };

    Some(lock.lock_owned().await)
}

/// Baseline `maxDepth` / `maxFanout` from the `advancedSettings` JSON
/// (`defaultSessionMaxDepth` / `defaultSessionMaxFanout`). A missing, non-numeric or `0` value
/// means unlimited, so it yields `None`.
pub fn baseline_limits_from_settings(settings_json: &str) -> (Option<u32>, Option<u32>) {
    let Ok(json) = serde_json::from_str::<serde_json::Value>(settings_json) else {
        return (None, None);
    };
    let limit = |key: &str| {
        json.get(key)
            .and_then(|value| value.as_u64())
            .and_then(|value| u32::try_from(value).ok())
            .filter(|value| *value > 0)
    };
    (
        limit("defaultSessionMaxDepth"),
        limit("defaultSessionMaxFanout"),
    )
}

/// Reads the baseline limits from the settings repository; unlimited when it is unavailable.
async fn baseline_limits() -> (Option<u32>, Option<u32>) {
    let Some(settings_repo) = crate::state::try_get_settings_repository() else {
        return (None, None);
    };
    match settings_repo.get("advancedSettings").await {
        Ok(Some(model)) => baseline_limits_from_settings(&model.value),
        _ => (None, None),
    }
}

/// The limit a new session ends up with: its own request, else what its parent carries, else the
/// baseline from the settings.
fn effective_limit(
    requested: Option<u32>,
    inherited: Option<u32>,
    baseline: Option<u32>,
) -> Option<u32> {
    requested.or(inherited).or(baseline)
}

/// True when `requested` does not raise the limit `inherited` allows. No inherited limit means
/// unlimited, so any request (which can only tighten it) is fine, and so is no request.
pub fn request_within_limit(requested: Option<u32>, inherited: Option<u32>) -> bool {
    match (requested, inherited) {
        (Some(requested), Some(inherited)) => requested <= inherited,
        _ => true,
    }
}

/// The `maxDepth` / `maxFanout` a child of `parent_id` inherits: what the parent carries, else
/// the baseline from the settings.
pub async fn inherited_limits_for(parent_id: &str) -> (Option<u32>, Option<u32>) {
    let from_cache = {
        let store = lineage_store().read().await;
        store
            .get(parent_id)
            .map(|meta| (meta.max_depth, meta.max_fanout))
    };
    let (parent_depth, parent_fanout) = match from_cache {
        Some(limits) => limits,
        None => crate::state::get_session_repository()
            .get_session(parent_id)
            .await
            .ok()
            .flatten()
            .map(|meta| (meta.max_depth, meta.max_fanout))
            .unwrap_or((None, None)),
    };
    let (baseline_depth, baseline_fanout) = baseline_limits().await;
    (
        parent_depth.or(baseline_depth),
        parent_fanout.or(baseline_fanout),
    )
}

pub async fn remove_lineage(session_id: &str) {
    lineage_store().write().await.remove(session_id);
    match parent_spawn_locks().lock() {
        Ok(mut locks) => {
            locks.remove(session_id);
        }
        Err(error) => {
            log::warn!(
                "Failed to clean parent spawn lock entry for {}: {}",
                session_id,
                error
            );
        }
    }
}

pub fn normalize_explicit_org(
    org_id: Option<String>,
    org_name: Option<String>,
    org_root_session_id: Option<String>,
) -> Result<Option<(String, String, String)>, String> {
    match (org_id, org_name, org_root_session_id) {
        (None, None, None) => Ok(None),
        (Some(org_id), Some(org_name), Some(org_root_session_id)) => {
            let org_id = org_id.trim().to_string();
            let org_name = org_name.trim().to_string();
            let org_root_session_id = org_root_session_id.trim().to_string();

            if org_id.is_empty() || org_name.is_empty() || org_root_session_id.is_empty() {
                return Err(
                    "Explicit org metadata must include non-empty orgId, orgName, and orgRootSessionId together"
                        .to_string(),
                );
            }

            Ok(Some((org_id, org_name, org_root_session_id)))
        }
        _ => Err(
            "Explicit org metadata must include orgId, orgName, and orgRootSessionId together"
                .to_string(),
        ),
    }
}

pub fn resolve_child_session_model_provider(
    requested_model: Option<String>,
    requested_provider: Option<String>,
    parent_session: Option<&SessionMetadata>,
) -> Result<(Option<String>, Option<String>), String> {
    let inherited_model = parent_session.map(|session| session.model.clone());
    let inherited_provider = parent_session.map(|session| session.provider.clone());

    if parent_session.is_none() && (requested_model.is_some() != requested_provider.is_some()) {
        return Err(
            "Child session model/provider override must include both model and provider when no parent session is available for inheritance"
                .to_string(),
        );
    }

    Ok((
        requested_model.or(inherited_model),
        requested_provider.or(inherited_provider),
    ))
}

pub async fn resolve_spawn_lineage(
    session_id: &str,
    request: &CreateSessionRequest,
    explicit_org: Option<&(String, String, String)>,
) -> Result<SessionLineageMeta, String> {
    let requested_max_depth = request.max_depth;
    let requested_max_fanout = request.max_fanout;
    let (baseline_max_depth, baseline_max_fanout) = baseline_limits().await;

    if let Some(parent_id) = request.parent_session_id.as_deref() {
        // Clone parent fields under the read lock, then drop before any await
        // (SQLite child-count query inside `build_child_lineage_meta`).
        let cached_seed = {
            let store = lineage_store().read().await;
            store.get(parent_id).map(|parent_meta| ChildLineageSeed {
                parent_id: parent_id.to_string(),
                lineage_id: parent_meta.lineage_id.clone(),
                parent_depth: parent_meta.depth,
                inherited_max_depth: parent_meta.max_depth,
                inherited_max_fanout: parent_meta.max_fanout,
            })
        };

        if let Some(seed) = cached_seed {
            return build_child_lineage_meta(
                seed,
                requested_max_depth,
                requested_max_fanout,
                (baseline_max_depth, baseline_max_fanout),
                explicit_org,
            )
            .await;
        }

        let session_repo = crate::state::get_session_repository();
        let parent_meta = session_repo.get_session(parent_id).await.ok().flatten();
        let parent_depth = parent_meta.as_ref().and_then(|m| m.depth).unwrap_or(0);
        let parent_lineage_id = parent_meta
            .as_ref()
            .and_then(|m| m.lineage_id.clone())
            .unwrap_or_else(|| parent_id.to_string());
        let inherited_max_depth = parent_meta.as_ref().and_then(|m| m.max_depth);
        let inherited_max_fanout = parent_meta.as_ref().and_then(|m| m.max_fanout);

        return build_child_lineage_meta(
            ChildLineageSeed {
                parent_id: parent_id.to_string(),
                lineage_id: parent_lineage_id,
                parent_depth,
                inherited_max_depth,
                inherited_max_fanout,
            },
            requested_max_depth,
            requested_max_fanout,
            (baseline_max_depth, baseline_max_fanout),
            explicit_org,
        )
        .await;
    }

    Ok(SessionLineageMeta {
        parent_session_id: None,
        lineage_id: session_id.to_string(),
        depth: 0,
        max_depth: effective_limit(requested_max_depth, None, baseline_max_depth),
        max_fanout: effective_limit(requested_max_fanout, None, baseline_max_fanout),
        org_id: explicit_org.map(|(org_id, _, _)| org_id.clone()),
        org_name: explicit_org.map(|(_, org_name, _)| org_name.clone()),
        org_root_session_id: explicit_org
            .map(|(_, _, org_root_session_id)| org_root_session_id.clone()),
    })
}

struct ChildLineageSeed {
    parent_id: String,
    lineage_id: String,
    parent_depth: u32,
    inherited_max_depth: Option<u32>,
    inherited_max_fanout: Option<u32>,
}

async fn build_child_lineage_meta(
    seed: ChildLineageSeed,
    requested_max_depth: Option<u32>,
    requested_max_fanout: Option<u32>,
    baseline: (Option<u32>, Option<u32>),
    explicit_org: Option<&(String, String, String)>,
) -> Result<SessionLineageMeta, String> {
    let ChildLineageSeed {
        parent_id,
        lineage_id,
        parent_depth,
        inherited_max_depth,
        inherited_max_fanout,
    } = seed;
    let effective_max_depth = effective_limit(requested_max_depth, inherited_max_depth, baseline.0);
    let effective_max_fanout =
        effective_limit(requested_max_fanout, inherited_max_fanout, baseline.1);
    let next_depth = parent_depth.saturating_add(1);
    let child_count = crate::state::get_session_repository()
        .get_child_session_ids(&parent_id)
        .await
        .map(|children| children.len())
        .unwrap_or(0);

    if let Some(limit) = effective_max_depth {
        if next_depth > limit {
            return Err(format!(
                "Depth limit exceeded: next depth {} > maxDepth {}",
                next_depth, limit
            ));
        }
    }

    if let Some(limit) = effective_max_fanout {
        if child_count >= limit as usize {
            return Err(format!(
                "Fanout limit exceeded: parent has {} children, maxFanout is {}",
                child_count, limit
            ));
        }
    }

    Ok(SessionLineageMeta {
        parent_session_id: Some(parent_id),
        lineage_id,
        depth: next_depth,
        max_depth: effective_max_depth,
        max_fanout: effective_max_fanout,
        org_id: explicit_org.map(|(org_id, _, _)| org_id.clone()),
        org_name: explicit_org.map(|(_, org_name, _)| org_name.clone()),
        org_root_session_id: explicit_org
            .map(|(_, _, org_root_session_id)| org_root_session_id.clone()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn baseline_limits_come_from_advanced_settings_and_zero_means_unlimited() {
        assert_eq!(
            baseline_limits_from_settings(
                r#"{"defaultSessionMaxDepth":3,"defaultSessionMaxFanout":8}"#
            ),
            (Some(3), Some(8))
        );
        // 0 is "unlimited" in the settings UI.
        assert_eq!(
            baseline_limits_from_settings(
                r#"{"defaultSessionMaxDepth":0,"defaultSessionMaxFanout":0}"#
            ),
            (None, None)
        );
        // One of the two missing, a non-number, a negative or an oversized value, broken JSON.
        assert_eq!(
            baseline_limits_from_settings(r#"{"defaultSessionMaxDepth":2}"#),
            (Some(2), None)
        );
        assert_eq!(
            baseline_limits_from_settings(
                r#"{"defaultSessionMaxDepth":"3","defaultSessionMaxFanout":-1}"#
            ),
            (None, None)
        );
        assert_eq!(
            baseline_limits_from_settings(r#"{"defaultSessionMaxDepth":99999999999}"#),
            (None, None)
        );
        assert_eq!(baseline_limits_from_settings("not json"), (None, None));
        assert_eq!(baseline_limits_from_settings("{}"), (None, None));
    }

    #[test]
    fn effective_limit_prefers_the_request_then_the_parent_then_the_baseline() {
        assert_eq!(effective_limit(Some(1), Some(2), Some(3)), Some(1));
        assert_eq!(effective_limit(None, Some(2), Some(3)), Some(2));
        assert_eq!(effective_limit(None, None, Some(3)), Some(3));
        assert_eq!(effective_limit(None, None, None), None);
    }

    #[test]
    fn a_request_may_tighten_but_not_raise_the_inherited_limit() {
        assert!(request_within_limit(None, Some(3)));
        assert!(request_within_limit(Some(3), Some(3)));
        assert!(request_within_limit(Some(1), Some(3)));
        assert!(!request_within_limit(Some(4), Some(3)));
        // Unlimited inherited: a request can only tighten it.
        assert!(request_within_limit(Some(99), None));
        assert!(request_within_limit(None, None));
    }
}
