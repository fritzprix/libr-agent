use crate::session::get_session_manager;
use serde::{Deserialize, Serialize};
use tauri::command;
use tracing::{error, info};

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionSwitchRequest {
    pub session_id: String,
    pub use_async: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionCreateRequest {
    pub session_id: Option<String>,
    pub use_pool: Option<bool>,
    pub isolation_level: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionCleanupRequest {
    pub max_age_hours: Option<u64>,
    pub keep_recent: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionResponse {
    pub success: bool,
    pub message: String,
    pub session_id: Option<String>,
    pub data: Option<serde_json::Value>,
}

/// Switch to a specific session
#[command]
pub async fn switch_session(request: SessionSwitchRequest) -> Result<SessionResponse, String> {
    info!("Switching to session: {}", request.session_id);

    let session_manager =
        get_session_manager().map_err(|e| format!("Failed to get session manager: {e}"))?;

    let result = if request.use_async.unwrap_or(true) {
        session_manager
            .set_session_async(request.session_id.clone())
            .await
    } else {
        session_manager.set_session(request.session_id.clone())
    };

    match result {
        Ok(_) => {
            info!("Successfully switched to session: {}", request.session_id);
            Ok(SessionResponse {
                success: true,
                message: format!("Switched to session: {}", request.session_id),
                session_id: Some(request.session_id),
                data: None,
            })
        }
        Err(e) => {
            error!("Failed to switch session: {}", e);
            Err(format!("Failed to switch session: {e}"))
        }
    }
}

/// Create a new session (optionally from pool)
#[command]
pub async fn create_session(request: SessionCreateRequest) -> Result<SessionResponse, String> {
    let session_manager =
        get_session_manager().map_err(|e| format!("Failed to get session manager: {e}"))?;

    let session_id = if request.use_pool.unwrap_or(false) {
        // Get or create from pool for instant switching
        session_manager
            .get_pooled_session()
            .await
            .map_err(|e| format!("Failed to get pooled session: {e}"))?
    } else if let Some(id) = request.session_id {
        // Create specific session
        session_manager
            .set_session_async(id.clone())
            .await
            .map_err(|e| format!("Failed to create session: {e}"))?;
        id
    } else {
        // Generate new session ID
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let new_id = format!("session-{timestamp}");
        session_manager
            .set_session_async(new_id.clone())
            .await
            .map_err(|e| format!("Failed to create session: {e}"))?;
        new_id
    };

    info!("Created session: {session_id}");

    Ok(SessionResponse {
        success: true,
        message: format!("Created session: {session_id}"),
        session_id: Some(session_id),
        data: None,
    })
}

/// Get current session information
#[command]
pub async fn get_current_session_info() -> Result<SessionResponse, String> {
    let session_manager =
        get_session_manager().map_err(|e| format!("Failed to get session manager: {e}"))?;

    let current_session = session_manager.get_current_session();

    Ok(SessionResponse {
        success: true,
        message: "Retrieved current session".to_string(),
        session_id: current_session.clone(),
        data: Some(serde_json::json!({
            "current_session": current_session,
            "workspace_dir": session_manager.get_session_workspace_dir().to_string_lossy()
        })),
    })
}

/// List all available sessions
#[command]
pub async fn list_all_sessions() -> Result<SessionResponse, String> {
    let session_manager =
        get_session_manager().map_err(|e| format!("Failed to get session manager: {e}"))?;

    let sessions = session_manager
        .list_sessions()
        .map_err(|e| format!("Failed to list sessions: {e}"))?;

    let active_sessions = session_manager
        .get_active_sessions()
        .map_err(|e| format!("Failed to get active sessions: {e}"))?;

    Ok(SessionResponse {
        success: true,
        message: format!("Found {} sessions", sessions.len()),
        session_id: None,
        data: Some(serde_json::json!({
            "sessions": sessions,
            "active_sessions": active_sessions,
            "total_count": sessions.len()
        })),
    })
}

/// Get session statistics
#[command]
pub async fn get_session_stats() -> Result<SessionResponse, String> {
    let session_manager =
        get_session_manager().map_err(|e| format!("Failed to get session manager: {e}"))?;

    let stats = session_manager
        .get_session_stats()
        .map_err(|e| format!("Failed to get session stats: {e}"))?;

    Ok(SessionResponse {
        success: true,
        message: "Retrieved session statistics".to_string(),
        session_id: None,
        data: Some(
            serde_json::to_value(stats).map_err(|e| format!("Failed to serialize stats: {e}"))?,
        ),
    })
}

/// Pre-allocate sessions for faster switching
#[command]
pub async fn pre_allocate_sessions(count: usize) -> Result<SessionResponse, String> {
    let session_manager =
        get_session_manager().map_err(|e| format!("Failed to get session manager: {e}"))?;

    let allocated_sessions = session_manager
        .pre_allocate_sessions(count)
        .await
        .map_err(|e| format!("Failed to pre-allocate sessions: {e}"))?;

    info!("Pre-allocated {} sessions", allocated_sessions.len());

    Ok(SessionResponse {
        success: true,
        message: format!("Pre-allocated {} sessions", allocated_sessions.len()),
        session_id: None,
        data: Some(serde_json::json!({
            "allocated_sessions": allocated_sessions,
            "count": allocated_sessions.len()
        })),
    })
}

/// Clean up old or unused sessions
#[command]
pub async fn cleanup_sessions(request: SessionCleanupRequest) -> Result<SessionResponse, String> {
    let session_manager =
        get_session_manager().map_err(|e| format!("Failed to get session manager: {e}"))?;

    let max_age_hours = request.max_age_hours.unwrap_or(24);
    let keep_recent = request.keep_recent.unwrap_or(5);

    let removed_count = session_manager
        .cleanup_old_sessions(max_age_hours, keep_recent)
        .await
        .map_err(|e| format!("Failed to cleanup sessions: {e}"))?;

    info!("Cleaned up {removed_count} old sessions");

    Ok(SessionResponse {
        success: true,
        message: format!("Cleaned up {removed_count} old sessions"),
        session_id: None,
        data: Some(serde_json::json!({
            "removed_count": removed_count,
            "max_age_hours": max_age_hours,
            "keep_recent": keep_recent
        })),
    })
}

/// Remove a specific session
#[command]
pub async fn remove_session(session_id: String) -> Result<SessionResponse, String> {
    let session_manager =
        get_session_manager().map_err(|e| format!("Failed to get session manager: {e}"))?;

    session_manager
        .remove_session(&session_id)
        .await
        .map_err(|e| format!("Failed to remove session: {e}"))?;

    info!("Removed session: {session_id}");

    Ok(SessionResponse {
        success: true,
        message: format!("Removed session: {session_id}"),
        session_id: Some(session_id),
        data: None,
    })
}

/// Get session isolation capabilities
#[command]
pub async fn get_isolation_capabilities() -> Result<SessionResponse, String> {
    // This would require access to WorkspaceServer, but for now we'll create a new isolation manager
    let isolation_manager = crate::session_isolation::SessionIsolationManager::new();
    let capabilities = isolation_manager.validate_isolation_capabilities().await;

    Ok(SessionResponse {
        success: true,
        message: "Retrieved isolation capabilities".to_string(),
        session_id: None,
        data: Some(
            serde_json::to_value(capabilities)
                .map_err(|e| format!("Failed to serialize capabilities: {e}"))?,
        ),
    })
}

/// Fast session switch - combines cleanup, pool allocation, and switching
#[command]
pub async fn fast_session_switch(
    target_session_id: Option<String>,
) -> Result<SessionResponse, String> {
    let session_manager =
        get_session_manager().map_err(|e| format!("Failed to get session manager: {e}"))?;

    // Step 1: Clean up old sessions in background (non-blocking)
    let _cleanup_task = {
        let session_manager_clone = session_manager.clone();
        tokio::spawn(async move {
            if let Err(e) = session_manager_clone.cleanup_old_sessions(6, 10).await {
                error!("Background session cleanup failed: {}", e);
            }
        })
    };

    // Step 2: Get or create target session
    let session_id = if let Some(id) = target_session_id {
        // Switch to specific session
        session_manager
            .set_session_async(id.clone())
            .await
            .map_err(|e| format!("Failed to switch to session: {e}"))?;
        id
    } else {
        // Get from pool or create new
        let pooled_session = session_manager
            .get_pooled_session()
            .await
            .map_err(|e| format!("Failed to get pooled session: {e}"))?;

        session_manager
            .set_session_async(pooled_session.clone())
            .await
            .map_err(|e| format!("Failed to switch to pooled session: {e}"))?;

        pooled_session
    };

    // Step 3: Pre-allocate more sessions for future use (background)
    let _prealloc_task = {
        let session_manager_clone = session_manager.clone();
        tokio::spawn(async move {
            if let Err(e) = session_manager_clone.pre_allocate_sessions(3).await {
                error!("Background session pre-allocation failed: {}", e);
            }
        })
    };

    info!("Fast switched to session: {session_id}");

    Ok(SessionResponse {
        success: true,
        message: format!("Fast switched to session: {session_id}"),
        session_id: Some(session_id.clone()),
        data: Some(serde_json::json!({
            "switched_to": session_id,
            "workspace_dir": session_manager.get_session_workspace_dir().to_string_lossy(),
            "background_tasks": "cleanup and pre-allocation running"
        })),
    })
}

// ============================================================================
// Legacy Session Commands (for backward compatibility)
// ============================================================================

/// Sets the currently active session.
#[tauri::command]
pub async fn set_current_session(session_id: String) -> Result<(), String> {
    get_session_manager()?.set_session(session_id)
}

/// Gets the ID of the currently active session.
#[tauri::command]
pub async fn get_current_session_legacy() -> Result<Option<String>, String> {
    Ok(get_session_manager()?.get_current_session())
}

/// Gets the absolute path to the workspace directory for the current session.
#[tauri::command]
pub async fn get_session_workspace_dir() -> Result<String, String> {
    let path = get_session_manager()?.get_session_workspace_dir();
    Ok(path.to_string_lossy().to_string())
}

/// Lists the IDs of all available sessions.
#[tauri::command]
pub async fn list_sessions_legacy() -> Result<Vec<String>, String> {
    get_session_manager()?.list_sessions()
}
