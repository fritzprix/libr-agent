use crate::services::{BrowserSession, InteractiveBrowserServer};
use log::{debug, error, info};
use serde::Deserialize;
use tauri::State;

/// Creates a new interactive browser session.
///
/// # Arguments
/// * `server` - The `InteractiveBrowserServer` state, managed by Tauri.
/// * `url` - The initial URL to open in the new browser session.
/// * `title` - An optional title for the session.
///
/// # Returns
/// A `Result` containing the unique session ID on success, or an error string on failure.
#[tauri::command]
pub async fn create_browser_session(
    server: State<'_, InteractiveBrowserServer>,
    url: String,
    title: Option<String>,
) -> Result<String, String> {
    info!("Command: create_browser_session called with URL: {url}");

    match server.create_browser_session(&url, title.as_deref()).await {
        Ok(session_id) => {
            info!("Browser session created successfully: {session_id}");
            Ok(session_id)
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

/// Clicks an element in a browser session identified by a CSS selector.
///
/// # Arguments
/// * `server` - The `InteractiveBrowserServer` state.
/// * `session_id` - The ID of the browser session.
/// * `selector` - The CSS selector of the element to click.
///
/// # Returns
/// A `Result` containing a success message or the script result, or an error string on failure.
#[tauri::command]
pub async fn click_element(
    server: State<'_, InteractiveBrowserServer>,
    session_id: String,
    selector: String,
) -> Result<String, String> {
    debug!("Command: click_element called - session: {session_id}, selector: {selector}");

    match server.click_element(&session_id, &selector).await {
        Ok(result) => {
            debug!("Element clicked successfully: {result}");
            Ok(result)
        }
        Err(e) => {
            error!("Failed to click element '{selector}' in session {session_id}: {e}");
            Err(e)
        }
    }
}

/// Inputs text into an element in a browser session.
///
/// # Arguments
/// * `server` - The `InteractiveBrowserServer` state.
/// * `session_id` - The ID of the browser session.
/// * `selector` - The CSS selector of the input element.
/// * `text` - The text to input into the element.
///
/// # Returns
/// A `Result` containing a success message or the script result, or an error string on failure.
#[tauri::command]
pub async fn input_text(
    server: State<'_, InteractiveBrowserServer>,
    session_id: String,
    selector: String,
    text: String,
) -> Result<String, String> {
    debug!(
        "Command: input_text called - session: {session_id}, selector: {selector}, text: {text}"
    );

    match server.input_text(&session_id, &selector, &text).await {
        Ok(result) => {
            debug!("Text input successful: {result}");
            Ok(result)
        }
        Err(e) => {
            error!("Failed to input text into '{selector}' in session {session_id}: {e}");
            Err(e)
        }
    }
}

/// Scrolls the page in a browser session by a given amount.
///
/// # Arguments
/// * `server` - The `InteractiveBrowserServer` state.
/// * `session_id` - The ID of the browser session.
/// * `x` - The horizontal scroll amount.
/// * `y` - The vertical scroll amount.
///
/// # Returns
/// A `Result` containing a success message, or an error string on failure.
#[tauri::command]
pub async fn scroll_page(
    server: State<'_, InteractiveBrowserServer>,
    session_id: String,
    x: i32,
    y: i32,
) -> Result<String, String> {
    debug!("Command: scroll_page called - session: {session_id}, x: {x}, y: {y}");

    match server.scroll_page(&session_id, x, y).await {
        Ok(result) => {
            debug!("Page scroll successful: {result}");
            Ok(result)
        }
        Err(e) => {
            error!("Failed to scroll page in session {session_id}: {e}");
            Err(e)
        }
    }
}

/// Gets the current URL of a browser session.
///
/// # Arguments
/// * `server` - The `InteractiveBrowserServer` state.
/// * `session_id` - The ID of the browser session.
///
/// # Returns
/// A `Result` containing the current URL, or an error string on failure.
#[tauri::command]
pub async fn get_current_url(
    server: State<'_, InteractiveBrowserServer>,
    session_id: String,
) -> Result<String, String> {
    debug!("Command: get_current_url called for session: {session_id}");

    match server.get_current_url(&session_id).await {
        Ok(url) => {
            debug!("Current URL retrieved: {url}");
            Ok(url)
        }
        Err(e) => {
            error!("Failed to get current URL for session {session_id}: {e}");
            Err(e)
        }
    }
}

/// Gets the title of the current page in a browser session.
///
/// # Arguments
/// * `server` - The `InteractiveBrowserServer` state.
/// * `session_id` - The ID of the browser session.
///
/// # Returns
/// A `Result` containing the page title, or an error string on failure.
#[tauri::command]
pub async fn get_page_title(
    server: State<'_, InteractiveBrowserServer>,
    session_id: String,
) -> Result<String, String> {
    debug!("Command: get_page_title called for session: {session_id}");

    match server.get_page_title(&session_id).await {
        Ok(title) => {
            debug!("Page title retrieved: {title}");
            Ok(title)
        }
        Err(e) => {
            error!("Failed to get page title for session {session_id}: {e}");
            Err(e)
        }
    }
}

/// Checks if an element exists in a browser session.
///
/// # Arguments
/// * `server` - The `InteractiveBrowserServer` state.
/// * `session_id` - The ID of the browser session.
/// * `selector` - The CSS selector of the element to check.
///
/// # Returns
/// A `Result` containing `true` if the element exists, `false` otherwise, or an error string on failure.
#[tauri::command]
pub async fn element_exists(
    server: State<'_, InteractiveBrowserServer>,
    session_id: String,
    selector: String,
) -> Result<bool, String> {
    debug!("Command: element_exists called - session: {session_id}, selector: {selector}");

    match server.element_exists(&session_id, &selector).await {
        Ok(exists) => {
            debug!("Element existence check: {selector} = {exists}");
            Ok(exists)
        }
        Err(e) => {
            error!("Failed to check element existence '{selector}' in session {session_id}: {e}");
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

/// Gets the full HTML content of the current page in a browser session.
///
/// # Arguments
/// * `server` - The `InteractiveBrowserServer` state.
/// * `session_id` - The ID of the browser session.
///
/// # Returns
/// A `Result` containing the page's HTML content as a string, or an error string on failure.
#[tauri::command]
pub async fn get_page_content(
    server: State<'_, InteractiveBrowserServer>,
    session_id: String,
) -> Result<String, String> {
    debug!("Command: get_page_content called for session: {session_id}");

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
            error!("Failed to get page content for session {session_id}: {e}");
            Err(e)
        }
    }
}

/// Takes a screenshot of the current page in a browser session.
///
/// # Arguments
/// * `server` - The `InteractiveBrowserServer` state.
/// * `session_id` - The ID of the browser session.
///
/// # Returns
/// A `Result` containing the path to the saved screenshot, or an error string on failure.
#[tauri::command]
pub async fn take_screenshot(
    server: State<'_, InteractiveBrowserServer>,
    session_id: String,
) -> Result<String, String> {
    debug!("Command: take_screenshot called for session: {session_id}");

    match server.take_screenshot(&session_id).await {
        Ok(result) => {
            debug!("Screenshot taken successfully");
            Ok(result)
        }
        Err(e) => {
            error!("Failed to take screenshot for session {session_id}: {e}");
            Err(e)
        }
    }
}

/// Represents the payload received from the frontend when a browser script finishes executing.
#[derive(Deserialize)]
pub struct BrowserScriptPayload {
    #[serde(rename = "sessionId")]
    session_id: String,
    #[serde(rename = "requestId")]
    request_id: String,
    result: String,
}

/// Receives the result of a JavaScript execution from the webview and stores it for polling.
///
/// # Arguments
/// * `payload` - The `BrowserScriptPayload` containing the session ID, request ID, and result.
/// * `server` - The `InteractiveBrowserServer` state.
///
/// # Returns
/// An empty `Result` on success, or an error string on failure.
#[tauri::command]
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

/// Executes JavaScript in a browser session and returns a request ID for polling the result.
///
/// # Arguments
/// * `server` - The `InteractiveBrowserServer` state.
/// * `session_id` - The ID of the browser session.
/// * `script` - The JavaScript code to execute.
///
/// # Returns
/// A `Result` containing a unique request ID for polling, or an error string on failure.
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
        Ok(request_id) => {
            debug!("Script execution initiated, request_id: {request_id}");
            Ok(request_id)
        }
        Err(e) => {
            error!("Failed to execute script in session {session_id}: {e}");
            Err(e)
        }
    }
}

/// Polls for the result of an asynchronous script execution using its request ID.
///
/// # Arguments
/// * `server` - The `InteractiveBrowserServer` state.
/// * `request_id` - The ID of the script execution request to poll.
///
/// # Returns
/// A `Result` containing an `Option<String>`. `Some(result)` if the script has completed,
/// `None` if it's still pending, or an error string on failure.
#[tauri::command]
pub async fn poll_script_result(
    server: State<'_, InteractiveBrowserServer>,
    request_id: String,
) -> Result<Option<String>, String> {
    debug!("Polling for script result with request_id: {request_id}");

    server.poll_script_result(&request_id).await
}

/// Navigates the browser back to the previous page in the history.
///
/// # Arguments
/// * `server` - The `InteractiveBrowserServer` state.
/// * `session_id` - The ID of the browser session.
///
/// # Returns
/// A `Result` containing the script request ID, or an error string on failure.
#[tauri::command]
pub async fn navigate_back(
    server: State<'_, InteractiveBrowserServer>,
    session_id: String,
) -> Result<String, String> {
    debug!("Command: navigate_back called for session: {session_id}");

    match server
        .execute_script(&session_id, "history.back(); 'Navigated back'")
        .await
    {
        Ok(request_id) => Ok(request_id),
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
/// A `Result` containing the script request ID, or an error string on failure.
#[tauri::command]
pub async fn navigate_forward(
    server: State<'_, InteractiveBrowserServer>,
    session_id: String,
) -> Result<String, String> {
    debug!("Command: navigate_forward called for session: {session_id}");

    match server
        .execute_script(&session_id, "history.forward(); 'Navigated forward'")
        .await
    {
        Ok(request_id) => Ok(request_id),
        Err(e) => {
            error!("Failed to navigate forward in session {session_id}: {e}");
            Err(e)
        }
    }
}

/// Gets the text content of an element in a browser session.
///
/// # Arguments
/// * `server` - The `InteractiveBrowserServer` state.
/// * `session_id` - The ID of the browser session.
/// * `selector` - The CSS selector of the element.
///
/// # Returns
/// A `Result` containing the script request ID, or an error string on failure.
#[tauri::command]
pub async fn get_element_text(
    server: State<'_, InteractiveBrowserServer>,
    session_id: String,
    selector: String,
) -> Result<String, String> {
    debug!("Command: get_element_text called - session: {session_id}, selector: {selector}");

    let script = format!(
        "const el = document.querySelector('{}'); el ? el.textContent.trim() : null",
        selector.replace('\'', "\\'")
    );

    match server.execute_script(&session_id, &script).await {
        Ok(request_id) => Ok(request_id),
        Err(e) => {
            error!("Failed to get element text '{selector}' in session {session_id}: {e}");
            Err(e)
        }
    }
}

/// Gets the value of a specific attribute from an element in a browser session.
///
/// # Arguments
/// * `server` - The `InteractiveBrowserServer` state.
/// * `session_id` - The ID of the browser session.
/// * `selector` - The CSS selector of the element.
/// * `attribute` - The name of the attribute to get.
///
/// # Returns
/// A `Result` containing the script request ID, or an error string on failure.
#[tauri::command]
pub async fn get_element_attribute(
    server: State<'_, InteractiveBrowserServer>,
    session_id: String,
    selector: String,
    attribute: String,
) -> Result<String, String> {
    debug!(
        "Command: get_element_attribute called - session: {session_id}, selector: {selector}, attribute: {attribute}"
    );

    let script = format!(
        "const el = document.querySelector('{}'); el ? el.getAttribute('{}') : null",
        selector.replace('\'', "\\'"),
        attribute.replace('\'', "\\'")
    );

    match server.execute_script(&session_id, &script).await {
        Ok(request_id) => Ok(request_id),
        Err(e) => {
            error!(
                "Failed to get element attribute '{attribute}' for '{selector}' in session {session_id}: {e}"
            );
            Err(e)
        }
    }
}

/// Finds an element in a browser session and returns detailed information about it.
///
/// # Arguments
/// * `server` - The `InteractiveBrowserServer` state.
/// * `session_id` - The ID of the browser session.
/// * `selector` - The CSS selector of the element to find.
///
/// # Returns
/// A `Result` containing the script request ID. The script result will be a JSON string
/// with details about the element (e.g., visibility, position, attributes).
#[tauri::command]
pub async fn find_element(
    server: State<'_, InteractiveBrowserServer>,
    session_id: String,
    selector: String,
) -> Result<String, String> {
    debug!("Command: find_element called - session: {session_id}, selector: {selector}");

    let script = format!(
        r#"
(function() {{
  const selector = '{}';
  try {{
    const el = document.querySelector(selector);
    if (!el) return JSON.stringify({{ exists: false, selector }});

    const rect = el.getBoundingClientRect();
    const style = window.getComputedStyle(el);
    const visible = !!(rect.width > 0 && rect.height > 0 && style.display !== 'none' && style.visibility !== 'hidden');
    const clickable = visible && style.pointerEvents !== 'none' && !el.disabled;

    return JSON.stringify({{
      exists: true,
      visible,
      clickable,
      tagName: el.tagName.toLowerCase(),
      rect: {{ x: rect.x, y: rect.y, width: rect.width, height: rect.height }},
      attributes: {{
        id: el.id || null,
        className: el.className || null,
        disabled: el.disabled || false
      }},
      selector
    }});
  }} catch (error) {{
    return JSON.stringify({{ exists: false, error: error.message, selector }});
  }}
}})()
"#,
        selector.replace('\'', "\\'")
    );

    match server.execute_script(&session_id, &script).await {
        Ok(request_id) => Ok(request_id),
        Err(e) => {
            error!("Failed to find element '{selector}' in session {session_id}: {e}");
            Err(e)
        }
    }
}
