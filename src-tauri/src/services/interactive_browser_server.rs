use chrono::{DateTime, Utc};

use log::{debug, error, info};

use serde::{Deserialize, Serialize};

use std::collections::HashMap;

use std::sync::{Arc, Mutex, RwLock};

use tauri::{AppHandle, Listener, Manager, WebviewUrl, WebviewWindowBuilder};

use tokio::sync::oneshot;

use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]

pub struct BrowserSession {
    pub id: String,

    pub window_label: String,

    pub url: String,

    pub title: String,

    pub created_at: DateTime<Utc>,

    pub status: SessionStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]

pub enum SessionStatus {
    Creating,

    Active,

    Paused,

    Closed,

    Error(String),
}

#[derive(Debug, Clone)]

pub struct InteractiveBrowserServer {
    app_handle: AppHandle,

    sessions: Arc<RwLock<HashMap<String, BrowserSession>>>,

    result_waiters: Arc<Mutex<HashMap<String, oneshot::Sender<String>>>>,
}

impl InteractiveBrowserServer {
    pub fn new(app_handle: AppHandle) -> Self {
        info!("Initializing Interactive Browser Server");

        Self {
            app_handle,

            sessions: Arc::new(RwLock::new(HashMap::new())),

            result_waiters: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Create a new browser session using Tauri's multiwindow pattern

    pub async fn create_browser_session(
        &self,

        url: &str,

        title: Option<&str>,
    ) -> Result<String, String> {
        let session_id = Uuid::new_v4().to_string();

        let window_label = format!("browser-{}", session_id);

        let session_title = title.unwrap_or("Interactive Browser Agent");

        info!(
            "Creating new browser session: {} for URL: {}",
            session_id, url
        );

        // Create WebviewWindow (independent browser window)

        let webview_window = WebviewWindowBuilder::new(
            &self.app_handle,
            &window_label,
            WebviewUrl::External(url.parse().map_err(|e| format!("Invalid URL: {}", e))?),
        )
        .title(&format!(
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
        .map_err(|e| format!("Failed to create browser window: {}", e))?;

        // Register session

        let session = BrowserSession {
            id: session_id.clone(),

            window_label: window_label.clone(),

            url: url.to_string(),

            title: session_title.to_string(),

            created_at: Utc::now(),

            status: SessionStatus::Active,
        };

        {
            let mut sessions = self
                .sessions
                .write()
                .map_err(|e| format!("Failed to acquire write lock: {}", e))?;

            sessions.insert(session_id.clone(), session);
        }

        // Register window event listeners

        let sessions_clone = self.sessions.clone();

        let session_id_clone = session_id.clone();

        webview_window.once("tauri://close-requested", move |_| {
            debug!(
                "Browser window close requested for session: {}",
                session_id_clone
            );

            if let Ok(mut sessions) = sessions_clone.write() {
                if let Some(session) = sessions.get_mut(&session_id_clone) {
                    session.status = SessionStatus::Closed;

                    info!("Session {} marked as closed", session_id_clone);
                }
            }
        });

        info!("Browser session created successfully: {}", session_id);

        Ok(session_id)
    }

    /// Execute JavaScript in a browser session and return the result

    pub async fn execute_script(&self, session_id: &str, script: &str) -> Result<String, String> {
        debug!("Executing script in session {}: {}", session_id, script);

        let session = {
            let sessions = self
                .sessions
                .read()
                .map_err(|e| format!("Failed to acquire read lock: {}", e))?;

            sessions
                .get(session_id)
                .cloned()
                .ok_or("Session not found")?
        };

        if let Some(window) = self.app_handle.get_webview_window(&session.window_label) {
            // Create oneshot channel for result

            let (tx, rx) = oneshot::channel();

            // Store the sender in result_waiters

            {
                let mut waiters = self
                    .result_waiters
                    .lock()
                    .map_err(|e| format!("Failed to acquire result_waiters lock: {}", e))?;

                waiters.insert(session_id.to_string(), tx);
            }

            // Inject JS that calls window.__TAURI_INTERNALS__.invoke

            // ...
            // Inject JS that calls window.__TAURI_INTERNALS__.invoke
            let wrapped_script = format!(
                r#"
(async function() {{
    try {{
        const result = await (async () => {{ return {script}; }})();
        const resultStr = (typeof result === 'undefined' || result === null) 
            ? 'null' 
            : (typeof result === 'object' ? JSON.stringify(result) : String(result));
        
        const payload = {{ sessionId: '{session_id}', result: resultStr }};

        console.log('[TAURI INJECTION] Sending to browser_script_result:', payload);
        window.__TAURI__.core.invoke('browser_script_result', {{ payload }});
    }} catch (error) {{
        const errorStr = 'Error: ' + error.message;

        const payload = {{ sessionId: '{session_id}', result: errorStr }};

        console.log('[TAURI INJECTION] Sending to browser_script_result (error):', payload);
        window.__TAURI__.core.invoke('browser_script_result', {{ payload }});
    }}
}})();
"#,
                script = script,
                session_id = session_id
            );

            // Execute the wrapped script

            match window.eval(&wrapped_script) {
                Ok(_) => {
                    debug!("Script wrapper executed in session: {}", session_id);

                    // Wait for result (with timeout)

                    match tokio::time::timeout(std::time::Duration::from_secs(5), rx).await {
                        Ok(Ok(result)) => {
                            debug!(
                                "Received script result for session {}: {}",
                                session_id, result
                            );

                            Ok(result)
                        }

                        Ok(Err(_)) => {
                            error!(
                                "Channel closed before result received for session {}",
                                session_id
                            );

                            Err("Channel closed before result received".to_string())
                        }

                        Err(_) => {
                            error!(
                                "Timeout waiting for script result in session {}",
                                session_id
                            );

                            // Clean up the orphaned waiter

                            if let Ok(mut waiters) = self.result_waiters.lock() {
                                waiters.remove(session_id);
                            }

                            Err("Timeout waiting for script result".to_string())
                        }
                    }
                }

                Err(e) => {
                    error!(
                        "Failed to execute script wrapper in session {}: {}",
                        session_id, e
                    );

                    // Clean up the waiter if script execution failed

                    if let Ok(mut waiters) = self.result_waiters.lock() {
                        waiters.remove(session_id);
                    }

                    Err(format!("Failed to execute script: {}", e))
                }
            }
        } else {
            error!("Browser window not found for session: {}", session_id);

            Err("Browser window not found".to_string())
        }
    }

    /// Click on a DOM element

    pub async fn click_element(&self, session_id: &str, selector: &str) -> Result<String, String> {
        debug!("Clicking element '{}' in session {}", selector, session_id);

        let script = format!(
            r#"

(function() {{

try {{

const element = document.querySelector('{}');

if (element) {{

element.click();

return 'Element clicked successfully';

}} else {{

throw new Error('Element not found: {}');

}}

}} catch (error) {{

throw new Error('Click failed: ' + error.message);

}}

}})()

"#,
            selector.replace('"', r#"\""#),
            selector.replace('"', r#"\""#)
        );

        self.execute_script(session_id, &script).await
    }

    /// Input text into a form field

    pub async fn input_text(
        &self,

        session_id: &str,

        selector: &str,

        text: &str,
    ) -> Result<String, String> {
        debug!(
            "Inputting text '{}' into element '{}' in session {}",
            text, selector, session_id
        );

        let script = format!(
            r#"

(function() {{

try {{

const element = document.querySelector('{}');

if (element) {{

element.value = '{}';

element.dispatchEvent(new Event('input', {{ bubbles: true }}));

element.dispatchEvent(new Event('change', {{ bubbles: true }}));

return 'Text input successfully: ' + element.value;

}} else {{

throw new Error('Input element not found: {}');

}}

}} catch (error) {{

throw new Error('Input failed: ' + error.message);

}}

}})()

"#,
            selector.replace('"', r#"\""#),
            text.replace('"', r#"\""#).replace('\n', r#"\n"#),
            selector.replace('"', r#"\""#)
        );

        self.execute_script(session_id, &script).await
    }

    /// Scroll the page to specified coordinates

    pub async fn scroll_page(&self, session_id: &str, x: i32, y: i32) -> Result<String, String> {
        debug!("Scrolling page to ({}, {}) in session {}", x, y, session_id);

        let script = format!(
            "window.scrollTo({}, {}); 'Scrolled to ({}, {})'",
            x, y, x, y
        );

        self.execute_script(session_id, &script).await
    }

    /// Get current page URL

    pub async fn get_current_url(&self, session_id: &str) -> Result<String, String> {
        debug!("Getting current URL for session {}", session_id);

        let script = "window.location.href";

        self.execute_script(session_id, script).await
    }

    /// Get page title

    pub async fn get_page_title(&self, session_id: &str) -> Result<String, String> {
        debug!("Getting page title for session {}", session_id);

        let script = "document.title";

        self.execute_script(session_id, script).await
    }

    /// Check if a DOM element exists

    pub async fn element_exists(&self, session_id: &str, selector: &str) -> Result<bool, String> {
        debug!(
            "Checking if element '{}' exists in session {}",
            selector, session_id
        );

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

                debug!(
                    "Element '{}' exists: {} in session {}",
                    selector, exists, session_id
                );

                Ok(exists)
            }

            Err(_) => {
                debug!(
                    "Element '{}' does not exist in session {}",
                    selector, session_id
                );

                Ok(false)
            }
        }
    }

    /// Get all active sessions

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
                error!("Failed to list sessions: {}", e);

                Vec::new()
            }
        }
    }

    /// Close a browser session

    pub async fn close_session(&self, session_id: &str) -> Result<String, String> {
        info!("Closing browser session: {}", session_id);

        let session = {
            let sessions = self
                .sessions
                .read()
                .map_err(|e| format!("Failed to acquire read lock: {}", e))?;

            sessions
                .get(session_id)
                .cloned()
                .ok_or("Session not found")?
        };

        if let Some(window) = self.app_handle.get_webview_window(&session.window_label) {
            window
                .close()
                .map_err(|e| format!("Failed to close window: {}", e))?;

            info!("Browser window closed for session: {}", session_id);
        }

        // Remove from sessions map

        {
            let mut sessions = self
                .sessions
                .write()
                .map_err(|e| format!("Failed to acquire write lock: {}", e))?;

            sessions.remove(session_id);
        }

        info!("Session {} closed successfully", session_id);

        Ok("Session closed successfully".to_string())
    }

    /// Navigate to a new URL in an existing session

    pub async fn navigate_to_url(&self, session_id: &str, url: &str) -> Result<String, String> {
        info!("Navigating session {} to URL: {}", session_id, url);

        let session = {
            let sessions = self
                .sessions
                .read()
                .map_err(|e| format!("Failed to acquire read lock: {}", e))?;

            sessions
                .get(session_id)
                .cloned()
                .ok_or("Session not found")?
        };

        if let Some(_window) = self.app_handle.get_webview_window(&session.window_label) {
            let script = format!("window.location.href = '{}'; 'Navigated to {}'", url, url);

            self.execute_script(session_id, &script).await?;

            // Update session URL

            {
                let mut sessions = self
                    .sessions
                    .write()
                    .map_err(|e| format!("Failed to acquire write lock: {}", e))?;

                if let Some(session) = sessions.get_mut(session_id) {
                    session.url = url.to_string();
                }
            }

            info!("Successfully navigated session {} to {}", session_id, url);

            Ok(format!("Navigated to {}", url))
        } else {
            error!("Browser window not found for session: {}", session_id);

            Err("Browser window not found".to_string())
        }
    }

    /// Get page content (HTML)

    pub async fn get_page_content(&self, session_id: &str) -> Result<String, String> {
        debug!("Getting page content for session {}", session_id);

        let script = "document.documentElement.outerHTML";

        match self.execute_script(session_id, script).await {
            Ok(result) => {
                if result.contains("HTML content extracted successfully") {
                    // For now, return a placeholder that indicates HTML was extracted

                    // In a real implementation, we would need to capture the actual HTML

                    info!(
                        "HTML content extraction completed for session: {}",
                        session_id
                    );

                    Ok("<!DOCTYPE html><html><head><title>Page Content Extracted</title></head><body><h1>HTML Content Successfully Extracted</h1><p>The page content has been extracted but due to Tauri v2 limitations, the actual HTML content cannot be returned directly.</p></body></html>".to_string())
                } else {
                    Ok(result)
                }
            }

            Err(e) => Err(e),
        }
    }

    /// Take a screenshot of the page (placeholder for future implementation)

    pub async fn take_screenshot(&self, session_id: &str) -> Result<String, String> {
        debug!("Taking screenshot for session {}", session_id);

        // This would be implemented when screenshot capability is added

        Err("Screenshot functionality not yet implemented".to_string())
    }

    /// Handle script result from the browser (internal method for browser_script_result command)
    pub fn handle_script_result(&self, session_id: &str, result: String) -> Result<(), String> {
        if let Ok(mut waiters) = self.result_waiters.lock() {
            if let Some(tx) = waiters.remove(session_id) {
                if tx.send(result).is_err() {
                    debug!(
                        "Receiver for session {} dropped, likely due to timeout.",
                        session_id
                    );
                }
            } else {
                debug!(
                    "No waiting channel for session {} (likely timed out).",
                    session_id
                );
            }
        } else {
            return Err("Failed to acquire lock on result_waiters.".to_string());
        }
        Ok(())
    }
}
