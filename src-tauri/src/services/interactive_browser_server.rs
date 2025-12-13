use chrono::{DateTime, Utc};

use log::{debug, error, info};

use serde::{Deserialize, Serialize};

use std::collections::HashMap;

use std::sync::{Arc, RwLock};

use tauri::{AppHandle, Listener, Manager, WebviewUrl, WebviewWindowBuilder};

use dashmap::DashMap;

use tokio::sync::Notify;
use tokio::time::{timeout, Duration};
use uuid::Uuid;

use super::browser_error::BrowserError;

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
    /// Notification for page load completion (Navigation Blocking)
    #[serde(skip)]
    pub page_load_notify: Arc<Notify>,
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

// ==================================================================================
// JavaScript Templates
// ==================================================================================

/// The initialization script injected into every browser window.
/// It sets up the `window.__LIBR_AGENT__` global object which handles:
/// 1. Waiting for Tauri IPC to be ready (critical for Linux/WebKitGTK)
/// 2. Executing scripts safely
/// 3. Sending results back to the Rust backend
const INIT_SCRIPT: &str = r#"
(function() {
    if (window.__LIBR_AGENT__) return;

    window.__LIBR_AGENT__ = {
        // Wait for Tauri IPC to be ready
        waitForIPC: async function(retries = 50, interval = 100) {
            if (window.__TAURI__) return true;
            console.log('[LibrAgent] Waiting for Tauri IPC...');
            for (let i = 0; i < retries; i++) {
                await new Promise(r => setTimeout(r, interval));
                if (window.__TAURI__) {
                    console.log('[LibrAgent] Tauri IPC is ready');
                    return true;
                }
            }
            console.error('[LibrAgent] Tauri IPC failed to initialize');
            return false;
        },

        // Helper: Send page load signal
        sendPageLoaded: async function() {
            if (!await this.waitForIPC()) return;
            try {
                // Get window label/session id from somewhere?
                // Actually the Rust side knows the session by the window that emits the command
                // But our protocol expects sessionId.
                // We don't easily have sessionId here unless we inject it.
                // However, 'create_browser_session' is what creates the window.
                // We can't easily inject sessionId into INIT_SCRIPT constant because it's static.
                // BUT, we can make the command `browser_page_loaded` work without sessionId if it infers from window label?
                // OR we can inject the session ID when we build the window?
                // `initialization_script` takes a string.
                // Wait, if we use `eval` later we can set it.
                // Ideally `browser_page_loaded` should rely on the window that called it.
                // The backend can map window -> session.
                await window.__TAURI__.core.invoke('browser_page_loaded', {}); 
            } catch (e) {
                console.error('[LibrAgent] Failed to send page load signal:', e);
            }
        },

        // Send result back to Rust

        // Send result back to Rust
        sendResult: async function(sessionId, requestId, result, isError = false) {
            if (!await this.waitForIPC()) {
                console.error('[LibrAgent] IPC not available, cannot send result');
                return;
            }
            
            const payload = {
                sessionId,
                requestId,
                result: isError ? `Error: ${result}` : (
                    typeof result === 'object' ? JSON.stringify(result) : String(result)
                )
            };
            
            try {
                await window.__TAURI__.core.invoke('browser_script_result', { payload });
            } catch (e) {
                console.error('[LibrAgent] Failed to invoke Tauri command:', e);
            }
        },

        // Execute user script safely
        // Note: This function uses 'new Function' which might be blocked by CSP.
        // For strict CSP environments, we should inject the script directly from Rust.
        execute: async function(sessionId, requestId, scriptContent) {
            console.log(`[LibrAgent] Executing request: ${requestId}`);
            try {
                // Use Function constructor to create an async function from the string
                const asyncFn = new Function('return (async () => { ' + scriptContent + ' })()');
                const result = await asyncFn();
                await this.sendResult(sessionId, requestId, result, false);
            } catch (e) {
                console.error('[LibrAgent] Script execution error:', e);
                await this.sendResult(sessionId, requestId, e.message, true);
            }
        }
    };
    
    console.log('[LibrAgent] Runtime initialized');
    
    // Explicitly signal that the page has loaded and runtime is ready
    // We wait for window.onload to ensure resources are loaded (better for CSR)
    if (document.readyState === 'complete') {
        window.__LIBR_AGENT__.sendPageLoaded();
    } else {
        window.addEventListener('load', () => {
             window.__LIBR_AGENT__.sendPageLoaded();
        }, { once: true });
        
        // Fallback: if load doesn't fire (already happened?), check periodically?
        // Actually, if we missed it, readyState would be complete.
        // But for safety, let's also set a max wait or check interactive state?
        // No, 'load' plus 'complete' check is standard. 
        // We'll add a safety timeout just in case generic load hangs on some resource.
        setTimeout(() => {
             if (document.readyState === 'interactive' || document.readyState === 'complete') {
                 console.log('[LibrAgent] Load event fallback trigger');
                 window.__LIBR_AGENT__.sendPageLoaded();
             }
        }, 10000); // 10s fallback
    }
})();
"#;

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

    /// Helper to apply platform-specific window settings (Linux focus fixes)
    fn apply_platform_window_settings(&self, window: &tauri::WebviewWindow) {
        #[cfg(not(target_os = "linux"))]
        let _ = window;

        #[cfg(target_os = "linux")]
        {
            use log::error;
            if let Err(e) = window.show() {
                error!("Failed to show window on Linux: {e}");
            }
            if let Err(e) = window.set_focus() {
                error!("Failed to focus window on Linux: {e}");
            }
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
        .focused(true)
        .visible(true)
        .devtools(cfg!(debug_assertions))
        .initialization_script(INIT_SCRIPT) // Inject robust initialization script
        .accept_first_mouse(true)
        .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 LibrAgent Browser")
        .build()
        .map_err(|e| format!("Failed to create browser window: {e}"))?;

        // Apply platform-specific settings (Linux focus, etc.)
        self.apply_platform_window_settings(&webview_window);

        // Register session

        let session = BrowserSession {
            id: session_id.clone(),

            window_label: window_label.clone(),

            url: url.to_string(),

            created_at: Utc::now(),

            status: SessionStatus::Active,

            page_load_notify: Arc::new(Notify::new()),
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
            let sessions = self.sessions.read().map_err(|e| {
                String::from(BrowserError::LockFailed {
                    reason: format!("Failed to acquire read lock: {e}"),
                })
            })?;

            sessions.get(session_id).cloned().ok_or_else(|| {
                String::from(BrowserError::SessionNotFound {
                    session_id: session_id.to_string(),
                })
            })?
        };

        if let Some(window) = self.app_handle.get_webview_window(&session.window_label) {
            // Generate unique request ID
            let request_id = Uuid::new_v4().to_string();

            // Ensure window is focused on Linux before execution
            self.apply_platform_window_settings(&window);

            // Construct the full script to inject directly
            // This avoids using 'new Function' inside the browser context, bypassing CSP restrictions
            let execution_call = format!(
                r#"
(async function() {{
    if (!window.__LIBR_AGENT__) {{
        console.error('[LibrAgent] Runtime not initialized');
        return;
    }}
    
    try {{
        // Wait for IPC first
        if (!await window.__LIBR_AGENT__.waitForIPC()) {{
            return;
        }}

        // Execute user script directly here
        const result = await (async () => {{ return {script}; }})();
        
        // Send result using helper
        await window.__LIBR_AGENT__.sendResult('{session_id}', '{request_id}', result, false);
    }} catch (e) {{
        console.error('[LibrAgent] Script execution error:', e);
        await window.__LIBR_AGENT__.sendResult('{session_id}', '{request_id}', e.message, true);
    }}
}})();
"#
            );

            // Execute the call
            match window.eval(&execution_call) {
                Ok(_) => {
                    debug!(
                        "Script execution initiated in session: {session_id}, request_id: {request_id}"
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
            let sessions = self.sessions.read().map_err(|e| {
                String::from(BrowserError::LockFailed {
                    reason: format!("Failed to acquire read lock: {e}"),
                })
            })?;

            sessions.get(session_id).cloned().ok_or_else(|| {
                String::from(BrowserError::SessionNotFound {
                    session_id: session_id.to_string(),
                })
            })?
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

    /// Navigates a browser session to a new URL.
    ///
    /// # Arguments
    /// * `session_id` - The ID of the session to navigate.
    /// * `url` - The URL to navigate to.
    ///
    /// # Returns
    /// An empty `Result` on success, or an error string on failure.
    pub async fn navigate_to_url(&self, session_id: &str, url: &str) -> Result<String, String> {
        info!("Navigating session {session_id} to {url}");

        let (window_label, notify) = {
            let mut sessions = self
                .sessions
                .write()
                .map_err(|e| format!("Failed to acquire write lock: {e}"))?;

            let session = sessions.get_mut(session_id).ok_or_else(|| {
                String::from(BrowserError::SessionNotFound {
                    session_id: session_id.to_string(),
                })
            })?;

            // Reset notification state for new navigation
            session.page_load_notify = Arc::new(Notify::new());

            // Update URL immediately? Or after?
            // Usually update after success, but here we want to reflect intent.
            session.url = url.to_string();

            (
                session.window_label.clone(),
                session.page_load_notify.clone(),
            )
        };

        if let Some(window) = self.app_handle.get_webview_window(&window_label) {
            // Use eval to set window.location.href
            let script = format!("window.location.href = '{}'", url.replace('\'', "\\'"));

            window
                .eval(&script)
                .map_err(|e| format!("Failed to navigate: {e}"))?;

            // BLOCKING WAIT for page load signal
            info!("Waiting for page load signal...");
            match timeout(Duration::from_secs(15), notify.notified()).await {
                Ok(_) => {
                    info!("Page load signal received for {}", url);
                    Ok(format!("Navigated to {}", url))
                }
                Err(_) => {
                    // Timeout occurred
                    error!("Timeout waiting for page load signal: {}", url);
                    // We return OK because navigation *did* happen, just timed out waiting for load.
                    // This allows the agent to try extracting anyway (which has its own checks).
                    Ok(format!(
                        "Navigated to {} (timeout waiting for load signal)",
                        url
                    ))
                }
            }
        } else {
            Err("Browser window not found".to_string())
        }
    }

    /// Handles the page loaded signal from the frontend.
    pub fn handle_page_loaded(&self, session_id: &str) {
        if let Ok(sessions) = self.sessions.read() {
            if let Some(session) = sessions.get(session_id) {
                session.page_load_notify.notify_one();
                debug!(
                    "Page loaded signal received/processed for session {}",
                    session_id
                );
            }
        }
    }

    /// Finds session ID by window label (helper for command)
    pub fn get_session_id_by_label(&self, label: &str) -> Option<String> {
        if let Ok(sessions) = self.sessions.read() {
            sessions
                .values()
                .find(|s| s.window_label == label)
                .map(|s| s.id.clone())
        } else {
            None
        }
    }

    /// Navigates a browser session back in history.
    pub async fn navigate_back(&self, session_id: &str) -> Result<String, String> {
        info!("Navigating back in session {session_id}");
        self.execute_simple_script(session_id, "history.back()")
            .await?;
        Ok("Navigating back".to_string())
    }

    /// Navigates a browser session forward in history.
    pub async fn navigate_forward(&self, session_id: &str) -> Result<String, String> {
        info!("Navigating forward in session {session_id}");
        self.execute_simple_script(session_id, "history.forward()")
            .await?;
        Ok("Navigating forward".to_string())
    }

    /// Helper to execute a simple script without waiting for a result (fire and forget).
    async fn execute_simple_script(&self, session_id: &str, script: &str) -> Result<(), String> {
        let session = {
            let sessions = self.sessions.read().map_err(|e| {
                String::from(BrowserError::LockFailed {
                    reason: format!("Failed to acquire read lock: {e}"),
                })
            })?;

            sessions.get(session_id).cloned().ok_or_else(|| {
                String::from(BrowserError::SessionNotFound {
                    session_id: session_id.to_string(),
                })
            })?
        };

        if let Some(window) = self.app_handle.get_webview_window(&session.window_label) {
            window
                .eval(script)
                .map_err(|e| format!("Failed to execute script: {e}"))
        } else {
            Err("Browser window not found".to_string())
        }
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
