use crate::services::{BrowserSession, InteractiveBrowserServer};
use log::{debug, error, info};
use serde::Deserialize;
use tauri::State;

#[tauri::command]
pub async fn create_browser_session(
    server: State<'_, InteractiveBrowserServer>,
    url: String,
    title: Option<String>,
) -> Result<String, String> {
    info!("Command: create_browser_session called with URL: {}", url);

    match server.create_browser_session(&url, title.as_deref()).await {
        Ok(session_id) => {
            info!("Browser session created successfully: {}", session_id);
            Ok(session_id)
        }
        Err(e) => {
            error!("Failed to create browser session: {}", e);
            Err(e)
        }
    }
}

#[tauri::command]
pub async fn close_browser_session(
    server: State<'_, InteractiveBrowserServer>,
    session_id: String,
) -> Result<String, String> {
    info!(
        "Command: close_browser_session called for session: {}",
        session_id
    );

    match server.close_session(&session_id).await {
        Ok(result) => {
            info!("Browser session closed successfully: {}", session_id);
            Ok(result)
        }
        Err(e) => {
            error!("Failed to close browser session {}: {}", session_id, e);
            Err(e)
        }
    }
}

#[tauri::command]
pub async fn click_element(
    server: State<'_, InteractiveBrowserServer>,
    session_id: String,
    selector: String,
) -> Result<String, String> {
    debug!(
        "Command: click_element called - session: {}, selector: {}",
        session_id, selector
    );

    match server.click_element(&session_id, &selector).await {
        Ok(result) => {
            debug!("Element clicked successfully: {}", result);
            Ok(result)
        }
        Err(e) => {
            error!(
                "Failed to click element '{}' in session {}: {}",
                selector, session_id, e
            );
            Err(e)
        }
    }
}

#[tauri::command]
pub async fn input_text(
    server: State<'_, InteractiveBrowserServer>,
    session_id: String,
    selector: String,
    text: String,
) -> Result<String, String> {
    debug!(
        "Command: input_text called - session: {}, selector: {}, text: {}",
        session_id, selector, text
    );

    match server.input_text(&session_id, &selector, &text).await {
        Ok(result) => {
            debug!("Text input successful: {}", result);
            Ok(result)
        }
        Err(e) => {
            error!(
                "Failed to input text into '{}' in session {}: {}",
                selector, session_id, e
            );
            Err(e)
        }
    }
}

#[tauri::command]
pub async fn scroll_page(
    server: State<'_, InteractiveBrowserServer>,
    session_id: String,
    x: i32,
    y: i32,
) -> Result<String, String> {
    debug!(
        "Command: scroll_page called - session: {}, x: {}, y: {}",
        session_id, x, y
    );

    match server.scroll_page(&session_id, x, y).await {
        Ok(result) => {
            debug!("Page scroll successful: {}", result);
            Ok(result)
        }
        Err(e) => {
            error!("Failed to scroll page in session {}: {}", session_id, e);
            Err(e)
        }
    }
}

#[tauri::command]
pub async fn get_current_url(
    server: State<'_, InteractiveBrowserServer>,
    session_id: String,
) -> Result<String, String> {
    debug!(
        "Command: get_current_url called for session: {}",
        session_id
    );

    match server.get_current_url(&session_id).await {
        Ok(url) => {
            debug!("Current URL retrieved: {}", url);
            Ok(url)
        }
        Err(e) => {
            error!(
                "Failed to get current URL for session {}: {}",
                session_id, e
            );
            Err(e)
        }
    }
}

#[tauri::command]
pub async fn get_page_title(
    server: State<'_, InteractiveBrowserServer>,
    session_id: String,
) -> Result<String, String> {
    debug!("Command: get_page_title called for session: {}", session_id);

    match server.get_page_title(&session_id).await {
        Ok(title) => {
            debug!("Page title retrieved: {}", title);
            Ok(title)
        }
        Err(e) => {
            error!("Failed to get page title for session {}: {}", session_id, e);
            Err(e)
        }
    }
}

#[tauri::command]
pub async fn element_exists(
    server: State<'_, InteractiveBrowserServer>,
    session_id: String,
    selector: String,
) -> Result<bool, String> {
    debug!(
        "Command: element_exists called - session: {}, selector: {}",
        session_id, selector
    );

    match server.element_exists(&session_id, &selector).await {
        Ok(exists) => {
            debug!("Element existence check: {} = {}", selector, exists);
            Ok(exists)
        }
        Err(e) => {
            error!(
                "Failed to check element existence '{}' in session {}: {}",
                selector, session_id, e
            );
            Err(e)
        }
    }
}

#[tauri::command]
pub async fn list_browser_sessions(
    server: State<'_, InteractiveBrowserServer>,
) -> Result<Vec<BrowserSession>, String> {
    debug!("Command: list_browser_sessions called");

    let sessions = server.list_sessions();
    info!("Listed {} active browser sessions", sessions.len());
    Ok(sessions)
}

#[tauri::command]
pub async fn navigate_to_url(
    server: State<'_, InteractiveBrowserServer>,
    session_id: String,
    url: String,
) -> Result<String, String> {
    info!(
        "Command: navigate_to_url called - session: {}, url: {}",
        session_id, url
    );

    match server.navigate_to_url(&session_id, &url).await {
        Ok(result) => {
            info!("Navigation successful: {}", result);
            Ok(result)
        }
        Err(e) => {
            error!(
                "Failed to navigate session {} to {}: {}",
                session_id, url, e
            );
            Err(e)
        }
    }
}

#[tauri::command]
pub async fn get_page_content(
    server: State<'_, InteractiveBrowserServer>,
    session_id: String,
) -> Result<String, String> {
    debug!(
        "Command: get_page_content called for session: {}",
        session_id
    );

    match server.get_page_content(&session_id).await {
        Ok(content) => {
            debug!(
                "Page content retrieved for session: {} (length: {})",
                session_id,
                content.len()
            );
            Ok(content)
        }
        Err(e) => {
            error!(
                "Failed to get page content for session {}: {}",
                session_id, e
            );
            Err(e)
        }
    }
}

#[tauri::command]
pub async fn take_screenshot(
    server: State<'_, InteractiveBrowserServer>,
    session_id: String,
) -> Result<String, String> {
    debug!(
        "Command: take_screenshot called for session: {}",
        session_id
    );

    match server.take_screenshot(&session_id).await {
        Ok(result) => {
            debug!("Screenshot taken successfully");
            Ok(result)
        }
        Err(e) => {
            error!(
                "Failed to take screenshot for session {}: {}",
                session_id, e
            );
            Err(e)
        }
    }
}

#[derive(Deserialize)]
pub struct BrowserScriptPayload {
    #[serde(rename = "sessionId")]
    session_id: String,
    #[serde(rename = "requestId")]
    request_id: String,
    result: String,
}

#[tauri::command]
/// Receives the JS execution result from the webview and stores it for polling.
pub async fn browser_script_result(
    payload: BrowserScriptPayload,
    server: State<'_, InteractiveBrowserServer>,
) -> Result<(), String> {
    debug!(
        "Received script result for session {}, request_id {}: {}",
        payload.session_id, payload.request_id, payload.result
    );

    server.handle_script_result(&payload.session_id, payload.request_id, payload.result)
}

#[tauri::command]
/// Execute JavaScript in a browser session and return request_id for polling
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
        Ok(request_id) => {
            debug!("Script execution initiated, request_id: {}", request_id);
            Ok(request_id)
        }
        Err(e) => {
            error!("Failed to execute script in session {}: {}", session_id, e);
            Err(e)
        }
    }
}

#[tauri::command]
/// Poll for a script result using request_id
pub async fn poll_script_result(
    server: State<'_, InteractiveBrowserServer>,
    request_id: String,
) -> Result<Option<String>, String> {
    debug!("Polling for script result with request_id: {}", request_id);

    server.poll_script_result(&request_id).await
}
