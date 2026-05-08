use crate::agent::types::{CreateSessionRequest, SessionLineageMeta};
use crate::repositories::session_repository::SessionRepository;
use crate::repositories::SessionMetadata;
use std::collections::HashMap;
use std::sync::OnceLock;
use tokio::sync::RwLock as TokioRwLock;

pub static SESSION_LINEAGE: OnceLock<TokioRwLock<HashMap<String, SessionLineageMeta>>> =
    OnceLock::new();

pub fn lineage_store() -> &'static TokioRwLock<HashMap<String, SessionLineageMeta>> {
    SESSION_LINEAGE.get_or_init(|| TokioRwLock::new(HashMap::new()))
}

pub async fn remove_lineage(session_id: &str) {
    lineage_store().write().await.remove(session_id);
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

    if let Some(parent_id) = request.parent_session_id.as_deref() {
        let store = lineage_store().read().await;
        if let Some(parent_meta) = store.get(parent_id) {
            return build_child_lineage_meta(
                ChildLineageSeed {
                    parent_id: parent_id.to_string(),
                    lineage_id: parent_meta.lineage_id.clone(),
                    parent_depth: parent_meta.depth,
                    inherited_max_depth: parent_meta.max_depth,
                    inherited_max_fanout: parent_meta.max_fanout,
                },
                requested_max_depth,
                requested_max_fanout,
                explicit_org,
            )
            .await;
        }
        drop(store);

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
            explicit_org,
        )
        .await;
    }

    Ok(SessionLineageMeta {
        parent_session_id: None,
        lineage_id: session_id.to_string(),
        depth: 0,
        max_depth: requested_max_depth,
        max_fanout: requested_max_fanout,
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
    explicit_org: Option<&(String, String, String)>,
) -> Result<SessionLineageMeta, String> {
    let ChildLineageSeed {
        parent_id,
        lineage_id,
        parent_depth,
        inherited_max_depth,
        inherited_max_fanout,
    } = seed;
    let effective_max_depth = requested_max_depth.or(inherited_max_depth);
    let effective_max_fanout = requested_max_fanout.or(inherited_max_fanout);
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
