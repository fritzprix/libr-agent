use crate::services::{BrowserSession, InteractiveBrowserServer};
use log::{debug, error, info};
use tauri::State;

use serde::Serialize;

#[derive(Serialize)]
pub struct CreateSessionResponse {
    pub session_id: String,
    pub message: String,
}

/// Creates a new interactive browser session.
///
/// # Arguments
/// * `server` - The `InteractiveBrowserServer` state, managed by Tauri.
/// * `url` - The initial URL to open in the new browser session.
/// * `title` - An optional title for the session.
///
/// # Returns
/// A `Result` containing the session response on success, or an error string on failure.
#[tauri::command]
pub async fn create_browser_session(
    server: State<'_, InteractiveBrowserServer>,
    url: String,
    title: Option<String>,
) -> Result<CreateSessionResponse, String> {
    info!("Command: create_browser_session called with URL: {url}");

    match server
        .create_browser_session(&url, title.as_deref(), true)
        .await
    {
        Ok((session_id, message)) => {
            info!("Browser session created successfully: {session_id}");
            Ok(CreateSessionResponse {
                session_id,
                message,
            })
        }
        Err(e) => {
            error!("Failed to create browser session: {e}");
            Err(e)
        }
    }
}

/// Closes an active browser session.
///
/// # Arguments
/// * `server` - The `InteractiveBrowserServer` state.
/// * `session_id` - The ID of the session to close.
///
/// # Returns
/// A `Result` containing a success message, or an error string on failure.
#[tauri::command]
pub async fn close_browser_session(
    server: State<'_, InteractiveBrowserServer>,
    session_id: String,
) -> Result<String, String> {
    info!("Command: close_browser_session called for session: {session_id}");

    match server.close_session(&session_id).await {
        Ok(result) => {
            info!("Browser session closed successfully: {session_id}");
            Ok(result)
        }
        Err(e) => {
            error!("Failed to close browser session {session_id}: {e}");
            Err(e)
        }
    }
}

/// Lists all active browser sessions.
///
/// # Arguments
/// * `server` - The `InteractiveBrowserServer` state.
///
/// # Returns
/// A `Result` containing a vector of `BrowserSession` objects, or an error string on failure.
#[tauri::command]
pub async fn list_browser_sessions(
    server: State<'_, InteractiveBrowserServer>,
) -> Result<Vec<BrowserSession>, String> {
    debug!("Command: list_browser_sessions called");

    let sessions = server.list_sessions();
    info!("Listed {} active browser sessions", sessions.len());
    Ok(sessions)
}

/// Navigates a browser session to a new URL.
///
/// # Arguments
/// * `server` - The `InteractiveBrowserServer` state.
/// * `session_id` - The ID of the browser session.
/// * `url` - The URL to navigate to.
///
/// # Returns
/// A `Result` containing a success message, or an error string on failure.
#[tauri::command]
pub async fn navigate_to_url(
    server: State<'_, InteractiveBrowserServer>,
    session_id: String,
    url: String,
) -> Result<String, String> {
    info!("Command: navigate_to_url called - session: {session_id}, url: {url}");

    match server.navigate_to_url(&session_id, &url).await {
        Ok(result) => {
            info!("Navigation successful: {result}");
            Ok(result)
        }
        Err(e) => {
            error!("Failed to navigate session {session_id} to {url}: {e}");
            Err(e)
        }
    }
}

/// Executes JavaScript in a browser session and returns the result directly.
/// Uses oneshot channel pattern with 30-second timeout. No polling required.
///
/// # Arguments
/// * `server` - The `InteractiveBrowserServer` state.
/// * `session_id` - The ID of the browser session.
/// * `script` - The JavaScript code to execute.
///
/// # Returns
/// A `Result` containing the script execution result string, or an error string on failure.
#[tauri::command]
pub async fn execute_script(
    server: State<'_, InteractiveBrowserServer>,
    session_id: String,
    script: String,
) -> Result<String, String> {
    debug!(
        "Command: execute_script called for session: {}, script length: {}",
        session_id,
        script.len()
    );

    match server.execute_script(&session_id, &script).await {
        Ok(result) => {
            debug!("Script execution completed successfully");
            Ok(result)
        }
        Err(e) => {
            error!("Failed to execute script in session {session_id}: {e}");
            Err(e)
        }
    }
}

/// Navigates the browser back to the previous page in the history.
///
/// # Arguments
/// * `server` - The `InteractiveBrowserServer` state.
/// * `session_id` - The ID of the browser session.
///
/// # Returns
/// A `Result` containing a success message, or an error string on failure.
#[tauri::command]
pub async fn navigate_back(
    server: State<'_, InteractiveBrowserServer>,
    session_id: String,
) -> Result<String, String> {
    debug!("Command: navigate_back called for session: {session_id}");

    match server.navigate_back(&session_id).await {
        Ok(result) => Ok(result),
        Err(e) => {
            error!("Failed to navigate back in session {session_id}: {e}");
            Err(e)
        }
    }
}

/// Navigates the browser forward to the next page in the history.
///
/// # Arguments
/// * `server` - The `InteractiveBrowserServer` state.
/// * `session_id` - The ID of the browser session.
///
/// # Returns
/// A `Result` containing a success message, or an error string on failure.
#[tauri::command]
pub async fn navigate_forward(
    server: State<'_, InteractiveBrowserServer>,
    session_id: String,
) -> Result<String, String> {
    debug!("Command: navigate_forward called for session: {session_id}");

    match server.navigate_forward(&session_id).await {
        Ok(result) => Ok(result),
        Err(e) => {
            error!("Failed to navigate forward in session {session_id}: {e}");
            Err(e)
        }
    }
}

/// Close browser sessions and delete the sticky agent Chromium profile (cookies/logins).
///
/// Does not affect everyday Chrome.
#[tauri::command]
pub async fn clear_agent_browser_data(
    server: State<'_, InteractiveBrowserServer>,
) -> Result<(), String> {
    info!("Command: clear_agent_browser_data");
    if let Err(error) = server.close_all_sessions().await {
        error!("Failed to close browser sessions before clearing agent profile: {error}");
        return Err(error);
    }
    // Chromium may briefly hold SingletonLock after close; retry the full
    // in-use check + delete (clear_agent_sticky_profile_dir) a few times.
    const CLEAR_ATTEMPTS: usize = 4;
    const CLEAR_RETRY_DELAY: std::time::Duration = std::time::Duration::from_millis(400);
    let mut last_error = None;
    for attempt in 1..=CLEAR_ATTEMPTS {
        match tokio::task::spawn_blocking(crate::browser_sidecar::clear_agent_sticky_profile_dir)
            .await
            .map_err(|e| format!("Clear agent browser data task failed: {e}"))?
        {
            Ok(()) => {
                if attempt > 1 {
                    info!("Cleared sticky agent browser profile after {attempt} attempts");
                } else {
                    info!("Cleared sticky agent browser profile");
                }
                return Ok(());
            }
            Err(error) => {
                last_error = Some(error);
                if attempt < CLEAR_ATTEMPTS {
                    tokio::time::sleep(CLEAR_RETRY_DELAY).await;
                }
            }
        }
    }
    let error = last_error.unwrap_or_else(|| "Failed to clear agent browser profile".to_string());
    error!("Failed to clear agent browser profile after {CLEAR_ATTEMPTS} attempts: {error}");
    Err(error)
}
