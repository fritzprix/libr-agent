use chrono::{DateTime, Utc};

use log::{debug, error, info};

use serde::{Deserialize, Serialize};

use std::collections::HashMap;

use std::sync::{Arc, RwLock};

use tauri::{AppHandle, Listener, Manager, WebviewUrl, WebviewWindowBuilder};

use dashmap::DashMap;

use uuid::Uuid;

/// Represents an interactive browser session, corresponding to a Tauri window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserSession {
    /// A unique identifier for the session.
    pub id: String,
    /// The label used by Tauri to identify the window.
    pub window_label: String,
    /// The current URL of the browser session.
    pub url: String,
    /// The timestamp of when the session was created.
    pub created_at: DateTime<Utc>,
    /// The current status of the session.
    pub status: SessionStatus,
}

/// Represents the status of a `BrowserSession`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionStatus {
    /// The session is in the process of being created.
    Creating,
    /// The session is active and ready for interaction.
    Active,
    /// The session is currently paused.
    Paused,
    /// The session has been closed.
    Closed,
    /// The session has encountered an error.
    Error(String),
}

/// Manages multiple interactive browser sessions.
/// This struct is managed as Tauri state and shared across commands.
#[derive(Debug, Clone)]
pub struct InteractiveBrowserServer {
    /// A handle to the Tauri application instance, used to create and manage windows.
    app_handle: AppHandle,
    /// A thread-safe map of active browser sessions, keyed by session ID.
    sessions: Arc<RwLock<HashMap<String, BrowserSession>>>,
    /// A thread-safe map to store the results of asynchronous script executions, keyed by request ID.
    script_results: Arc<DashMap<String, String>>,
}

impl InteractiveBrowserServer {
    /// Creates a new instance of the `InteractiveBrowserServer`.
    ///
    /// # Arguments
    /// * `app_handle` - A handle to the Tauri application instance.
    pub fn new(app_handle: AppHandle) -> Self {
        info!("Initializing Interactive Browser Server");

        Self {
            app_handle,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            script_results: Arc::new(DashMap::new()),
        }
    }

    /// Creates a new browser session by opening a new Tauri window.
    ///
    /// Each session is tracked in the `sessions` map and is associated with a unique window.
    ///
    /// # Arguments
    /// * `url` - The initial URL to load in the new window.
    /// * `title` - An optional title for the new window.
    ///
    /// # Returns
    /// A `Result` containing the unique session ID on success, or an error string on failure.
    pub async fn create_browser_session(
        &self,
        url: &str,
        title: Option<&str>,
    ) -> Result<String, String> {
        let session_id = Uuid::new_v4().to_string();

        let window_label = format!("browser-{session_id}");

        let session_title = title.unwrap_or("Interactive Browser Agent");

        info!("Creating new browser session: {session_id} for URL: {url}");

        // Create WebviewWindow (independent browser window)

        let webview_window = WebviewWindowBuilder::new(
            &self.app_handle,
            &window_label,
            WebviewUrl::External(url.parse().map_err(|e| format!("Invalid URL: {e}"))?),
        )
        .title(format!(
            "{} - {}",
            session_title,
            session_id[..8].to_uppercase()
        ))
        .inner_size(1200.0, 800.0)
        .resizable(true)
        .maximizable(true)
        .minimizable(true)
        .center()
        .build()
        .map_err(|e| format!("Failed to create browser window: {e}"))?;

        // Register session

        let session = BrowserSession {
            id: session_id.clone(),

            window_label: window_label.clone(),

            url: url.to_string(),

            created_at: Utc::now(),

            status: SessionStatus::Active,
        };

        {
            let mut sessions = self
                .sessions
                .write()
                .map_err(|e| format!("Failed to acquire write lock: {e}"))?;

            sessions.insert(session_id.clone(), session);
        }

        // Register window event listeners

        let sessions_clone = self.sessions.clone();

        let session_id_clone = session_id.clone();

        webview_window.once("tauri://close-requested", move |_| {
            debug!("Browser window close requested for session: {session_id_clone}");

            if let Ok(mut sessions) = sessions_clone.write() {
                if let Some(session) = sessions.get_mut(&session_id_clone) {
                    session.status = SessionStatus::Closed;

                    info!("Session {session_id_clone} marked as closed");
                }
            }
        });

        info!("Browser session created successfully: {session_id}");

        Ok(session_id)
    }

    /// Executes a given JavaScript snippet in a specific browser session's window.
    ///
    /// This method wraps the user-provided script in an async IIFE to handle promises
    /// and errors gracefully. It then sends the result (or error) back to the backend
    /// using the `browser_script_result` command, which can be polled by the frontend.
    ///
    /// # Arguments
    /// * `session_id` - The ID of the session in which to execute the script.
    /// * `script` - The JavaScript code to execute.
    ///
    /// # Returns
    /// A `Result` containing a unique `request_id` which can be used to poll for the
    /// script's result, or an error string on failure.
    pub async fn execute_script(&self, session_id: &str, script: &str) -> Result<String, String> {
        debug!("Executing script in session {session_id}: {script}");

        let session = {
            let sessions = self
                .sessions
                .read()
                .map_err(|e| format!("Failed to acquire read lock: {e}"))?;

            sessions
                .get(session_id)
                .cloned()
                .ok_or("Session not found")?
        };

        if let Some(window) = self.app_handle.get_webview_window(&session.window_label) {
            // Generate unique request ID
            let request_id = Uuid::new_v4().to_string();

            // Inject JS that calls window.__TAURI__.core.invoke with request_id
            let wrapped_script = format!(
                r#"
(async function() {{
    try {{
        const result = await (async () => {{ return {script}; }})();
        const resultStr = (typeof result === 'undefined' || result === null) 
            ? 'null' 
            : (typeof result === 'object' ? JSON.stringify(result) : String(result));
        
        const payload = {{ sessionId: '{session_id}', requestId: '{request_id}', result: resultStr }};

        console.log('[TAURI INJECTION] Sending to browser_script_result:', payload);
        window.__TAURI__.core.invoke('browser_script_result', {{ payload }});
    }} catch (error) {{
        const errorStr = 'Error: ' + error.message;

        const payload = {{ sessionId: '{session_id}', requestId: '{request_id}', result: errorStr }};

        console.log('[TAURI INJECTION] Sending to browser_script_result (error):', payload);
        window.__TAURI__.core.invoke('browser_script_result', {{ payload }});
    }}
}})();
"#
            );

            // Execute the wrapped script
            match window.eval(&wrapped_script) {
                Ok(_) => {
                    debug!(
                        "Script wrapper executed in session: {session_id}, request_id: {request_id}"
                    );
                    Ok(request_id) // Return request_id immediately
                }
                Err(e) => {
                    error!("Failed to execute script wrapper in session {session_id}: {e}");
                    Err(format!("Failed to execute script: {e}"))
                }
            }
        } else {
            error!("Browser window not found for session: {session_id}");
            Err("Browser window not found".to_string())
        }
    }

    /// Simulates a click on a DOM element in a browser session.
    ///
    /// This method constructs a JavaScript snippet that finds the element, gathers diagnostic
    /// information (visibility, disabled state, etc.), and attempts to click it.
    ///
    /// # Arguments
    /// * `session_id` - The ID of the session.
    /// * `selector` - The CSS selector for the element to click.
    ///
    /// # Returns
    /// A `Result` containing the script request ID.
    pub async fn click_element(&self, session_id: &str, selector: &str) -> Result<String, String> {
        debug!("Clicking element '{selector}' in session {session_id}");

        let script = format!(
            r#"
(async function() {{
  const ts = new Date().toISOString();
  const selector = '{}';
  
  try {{
    const el = document.querySelector(selector);
    if (!el) {{
      return JSON.stringify({{
        ok: false,
        action: 'click',
        reason: 'not_found',
        selector: selector,
        timestamp: ts
      }});
    }}

    // Get diagnostics
    const rect = el.getBoundingClientRect ? el.getBoundingClientRect() : null;
    const visible = !!(rect && rect.width > 0 && rect.height > 0);
    const disabled = el.disabled || el.hasAttribute('disabled');
    const computedStyle = window.getComputedStyle ? window.getComputedStyle(el) : null;
    const pointerEvents = computedStyle ? computedStyle.pointerEvents : 'auto';
    const visibility = computedStyle ? computedStyle.visibility : 'visible';

    const diagnostics = {{
      visible: visible,
      disabled: disabled,
      pointerEvents: pointerEvents,
      visibility: visibility,
      rect: rect ? {{ x: rect.x, y: rect.y, width: rect.width, height: rect.height }} : null
    }};

    // Try multiple click approaches
    try {{
      el.scrollIntoView({{ block: 'center', inline: 'center' }});
      el.focus();
      el.click();
      el.dispatchEvent(new MouseEvent('click', {{ bubbles: true, cancelable: true }}));
    }} catch (clickError) {{
      // Click attempts failed, but we still return diagnostics
    }}

    return JSON.stringify({{
      ok: true,
      action: 'click',
      selector: selector,
      timestamp: ts,
      clickAttempted: true,
      diagnostics: diagnostics,
      note: 'click attempted (handlers may ignore synthetic events)'
    }});
  }} catch (error) {{
    return JSON.stringify({{
      ok: false,
      action: 'click',
      error: String(error),
      selector: selector,
      timestamp: ts
    }});
  }}
}})()
"#,
            selector.replace('"', r#"\""#)
        );

        self.execute_script(session_id, &script).await
    }

    /// Inputs text into a form field in a browser session.
    ///
    /// This method constructs a JavaScript snippet that finds the element, checks if it's
    /// enabled, and then sets its value, dispatching appropriate events.
    ///
    /// # Arguments
    /// * `session_id` - The ID of the session.
    /// * `selector` - The CSS selector for the input element.
    /// * `text` - The text to input.
    ///
    /// # Returns
    /// A `Result` containing the script request ID.
    pub async fn input_text(
        &self,
        session_id: &str,
        selector: &str,
        text: &str,
    ) -> Result<String, String> {
        debug!("Inputting text '{text}' into element '{selector}' in session {session_id}");

        let script = format!(
            r#"
(async function() {{
  const ts = new Date().toISOString();
  const selector = '{}';
  const inputText = '{}';
  
  try {{
    const el = document.querySelector(selector);
    if (!el) {{
      return JSON.stringify({{
        ok: false,
        action: 'input',
        reason: 'not_found',
        selector: selector,
        timestamp: ts
      }});
    }}

    // Get diagnostics
    const rect = el.getBoundingClientRect ? el.getBoundingClientRect() : null;
    const visible = !!(rect && rect.width > 0 && rect.height > 0);
    const disabled = el.disabled || el.hasAttribute('disabled') || el.readOnly || el.hasAttribute('readonly');
    const computedStyle = window.getComputedStyle ? window.getComputedStyle(el) : null;
    const pointerEvents = computedStyle ? computedStyle.pointerEvents : 'auto';
    const visibility = computedStyle ? computedStyle.visibility : 'visible';

    const diagnostics = {{
      visible: visible,
      disabled: disabled,
      pointerEvents: pointerEvents,
      visibility: visibility,
      rect: rect ? {{ x: rect.x, y: rect.y, width: rect.width, height: rect.height }} : null,
      tagName: el.tagName.toLowerCase(),
      type: el.type || 'unknown'
    }};

    if (disabled) {{
      return JSON.stringify({{
        ok: false,
        action: 'input',
        reason: 'element_disabled',
        selector: selector,
        timestamp: ts,
        diagnostics: diagnostics
      }});
    }}

    // Try to input text
    let applied = false;
    try {{
      el.scrollIntoView({{ block: 'center', inline: 'center' }});
      el.focus();
      
      // Clear existing value and set new value
      el.value = '';
      el.value = inputText;
      
      // Dispatch events
      el.dispatchEvent(new Event('input', {{ bubbles: true, cancelable: true }}));
      el.dispatchEvent(new Event('change', {{ bubbles: true, cancelable: true }}));
      el.dispatchEvent(new KeyboardEvent('keyup', {{ bubbles: true, cancelable: true }}));
      
      applied = true;
    }} catch (inputError) {{
      return JSON.stringify({{
        ok: false,
        action: 'input',
        error: String(inputError),
        selector: selector,
        timestamp: ts,
        diagnostics: diagnostics
      }});
    }}

    const finalValue = el.value || '';
    const valuePreview = finalValue.length > 50 ? finalValue.substring(0, 50) + '...' : finalValue;

    return JSON.stringify({{
      ok: true,
      action: 'input',
      selector: selector,
      timestamp: ts,
      applied: applied,
      diagnostics: diagnostics,
      value_preview: valuePreview,
      note: 'input attempted (handlers may modify final value)'
    }});
  }} catch (error) {{
    return JSON.stringify({{
      ok: false,
      action: 'input',
      error: String(error),
      selector: selector,
      timestamp: ts
    }});
  }}
}})()
"#,
            selector.replace('"', r#"\""#),
            text.replace('"', r#"\""#).replace('\n', r#"\\n"#)
        );

        self.execute_script(session_id, &script).await
    }

    /// Scrolls the page to the specified coordinates.
    ///
    /// # Arguments
    /// * `session_id` - The ID of the session.
    /// * `x` - The horizontal coordinate to scroll to.
    /// * `y` - The vertical coordinate to scroll to.
    ///
    /// # Returns
    /// A `Result` containing the script request ID.
    pub async fn scroll_page(&self, session_id: &str, x: i32, y: i32) -> Result<String, String> {
        debug!("Scrolling page to ({x}, {y}) in session {session_id}");

        let script = format!("window.scrollTo({x}, {y}); 'Scrolled to ({x}, {y})'");

        self.execute_script(session_id, &script).await
    }

    /// Gets the current URL of the page in the specified session.
    ///
    /// # Arguments
    /// * `session_id` - The ID of the session.
    ///
    /// # Returns
    /// A `Result` containing the script request ID for polling the URL.
    pub async fn get_current_url(&self, session_id: &str) -> Result<String, String> {
        debug!("Getting current URL for session {session_id}");

        let script = "window.location.href";

        self.execute_script(session_id, script).await
    }

    /// Gets the title of the current page in the specified session.
    ///
    /// # Arguments
    /// * `session_id` - The ID of the session.
    ///
    /// # Returns
    /// A `Result` containing the script request ID for polling the title.
    pub async fn get_page_title(&self, session_id: &str) -> Result<String, String> {
        debug!("Getting page title for session {session_id}");

        let script = "document.title";

        self.execute_script(session_id, script).await
    }

    /// Checks if a DOM element exists for the given selector.
    ///
    /// # Arguments
    /// * `session_id` - The ID of the session.
    /// * `selector` - The CSS selector to check for.
    ///
    /// # Returns
    /// A `Result` containing `true` if the element exists, `false` otherwise.
    pub async fn element_exists(&self, session_id: &str, selector: &str) -> Result<bool, String> {
        debug!("Checking if element '{selector}' exists in session {session_id}");

        let script = format!(
            r#"

(function() {{

try {{

const element = document.querySelector('{}');

return element !== null;

}} catch (error) {{

return false;

}}

}})()

"#,
            selector.replace('"', r#"\""#)
        );

        match self.execute_script(session_id, &script).await {
            Ok(result) => {
                // Parse the result to determine if element exists

                let exists =
                    result.contains("true") || result.contains("Element clicked successfully");

                debug!("Element '{selector}' exists: {exists} in session {session_id}");

                Ok(exists)
            }

            Err(_) => {
                debug!("Element '{selector}' does not exist in session {session_id}");

                Ok(false)
            }
        }
    }

    /// Lists all currently active (not closed) browser sessions.
    ///
    /// # Returns
    /// A vector of `BrowserSession` structs.
    pub fn list_sessions(&self) -> Vec<BrowserSession> {
        match self.sessions.read() {
            Ok(sessions) => {
                let active_sessions: Vec<BrowserSession> = sessions
                    .values()
                    .filter(|session| !matches!(session.status, SessionStatus::Closed))
                    .cloned()
                    .collect();

                debug!("Listed {} active sessions", active_sessions.len());

                active_sessions
            }

            Err(e) => {
                error!("Failed to list sessions: {e}");

                Vec::new()
            }
        }
    }

    /// Closes a browser session, which includes closing the associated Tauri window
    /// and removing the session from the active sessions map.
    ///
    /// # Arguments
    /// * `session_id` - The ID of the session to close.
    ///
    /// # Returns
    /// A `Result` containing a success message, or an error string on failure.
    pub async fn close_session(&self, session_id: &str) -> Result<String, String> {
        info!("Closing browser session: {session_id}");

        let session = {
            let sessions = self
                .sessions
                .read()
                .map_err(|e| format!("Failed to acquire read lock: {e}"))?;

            sessions
                .get(session_id)
                .cloned()
                .ok_or("Session not found")?
        };

        if let Some(window) = self.app_handle.get_webview_window(&session.window_label) {
            window
                .close()
                .map_err(|e| format!("Failed to close window: {e}"))?;

            info!("Browser window closed for session: {session_id}");
        }

        // Remove from sessions map

        {
            let mut sessions = self
                .sessions
                .write()
                .map_err(|e| format!("Failed to acquire write lock: {e}"))?;

            sessions.remove(session_id);
        }

        info!("Session {session_id} closed successfully");

        Ok("Session closed successfully".to_string())
    }

    /// Navigates an existing browser session to a new URL.
    ///
    /// # Arguments
    /// * `session_id` - The ID of the session to navigate.
    /// * `url` - The new URL to load.
    ///
    /// # Returns
    /// A `Result` containing a success message, or an error string on failure.
    pub async fn navigate_to_url(&self, session_id: &str, url: &str) -> Result<String, String> {
        info!("Navigating session {session_id} to URL: {url}");

        let session = {
            let sessions = self
                .sessions
                .read()
                .map_err(|e| format!("Failed to acquire read lock: {e}"))?;

            sessions
                .get(session_id)
                .cloned()
                .ok_or("Session not found")?
        };

        if let Some(_window) = self.app_handle.get_webview_window(&session.window_label) {
            let script = format!("window.location.href = '{url}'; 'Navigated to {url}'");

            self.execute_script(session_id, &script).await?;

            // Update session URL

            {
                let mut sessions = self
                    .sessions
                    .write()
                    .map_err(|e| format!("Failed to acquire write lock: {e}"))?;

                if let Some(session) = sessions.get_mut(session_id) {
                    session.url = url.to_string();
                }
            }

            info!("Successfully navigated session {session_id} to {url}");

            Ok(format!("Navigated to {url}"))
        } else {
            error!("Browser window not found for session: {session_id}");

            Err("Browser window not found".to_string())
        }
    }

    /// Gets the full HTML content of the page in the specified session.
    ///
    /// # Arguments
    /// * `session_id` - The ID of the session.
    ///
    /// # Returns
    /// A `Result` containing the script request ID for polling the HTML content.
    pub async fn get_page_content(&self, session_id: &str) -> Result<String, String> {
        debug!("Getting page content for session {session_id}");

        let script = "document.documentElement.outerHTML";

        match self.execute_script(session_id, script).await {
            Ok(result) => {
                if result.contains("HTML content extracted successfully") {
                    // For now, return a placeholder that indicates HTML was extracted

                    // In a real implementation, we would need to capture the actual HTML

                    info!("HTML content extraction completed for session: {session_id}");

                    Ok("<!DOCTYPE html><html><head><title>Page Content Extracted</title></head><body><h1>HTML Content Successfully Extracted</h1><p>The page content has been extracted but due to Tauri v2 limitations, the actual HTML content cannot be returned directly.</p></body></html>".to_string())
                } else {
                    Ok(result)
                }
            }

            Err(e) => Err(e),
        }
    }

    /// Takes a screenshot of the page. (Placeholder for future implementation)
    ///
    /// # Arguments
    /// * `session_id` - The ID of the session.
    ///
    /// # Returns
    /// An `Err` as this feature is not yet implemented.
    pub async fn take_screenshot(&self, session_id: &str) -> Result<String, String> {
        debug!("Taking screenshot for session {session_id}");

        // This would be implemented when screenshot capability is added

        Err("Screenshot functionality not yet implemented".to_string())
    }

    /// Polls for the result of a script execution using its request ID.
    ///
    /// This method checks the `script_results` map for a result associated with the
    /// given `request_id`. If found, it returns the result and removes it from the map.
    ///
    /// # Arguments
    /// * `request_id` - The ID of the script execution request.
    ///
    /// # Returns
    /// A `Result` containing an `Option<String>`. `Some(result)` if the result is available,
    /// `None` if it is not yet available.
    pub async fn poll_script_result(&self, request_id: &str) -> Result<Option<String>, String> {
        if let Some((_key, result)) = self.script_results.remove(request_id) {
            debug!("Retrieved script result for request_id: {request_id}");
            Ok(Some(result))
        } else {
            Ok(None)
        }
    }

    /// Handles the script result received from the `browser_script_result` command.
    ///
    /// This method is called internally when the frontend sends back the result of a
    /// script execution. It stores the result in the `script_results` map.
    ///
    /// # Arguments
    /// * `session_id` - The ID of the session where the script was executed.
    /// * `request_id` - The unique ID of the script execution request.
    /// * `result` - The string result of the script execution.
    ///
    /// # Returns
    /// An empty `Result` on success.
    pub fn handle_script_result(
        &self,
        session_id: &str,
        request_id: String,
        result: String,
    ) -> Result<(), String> {
        debug!("Storing script result for session: {session_id}, request_id: {request_id}");
        self.script_results.insert(request_id, result);
        Ok(())
    }
}
