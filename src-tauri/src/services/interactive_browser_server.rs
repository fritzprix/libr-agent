use chrono::{DateTime, Utc};

use log::{debug, error, info, warn};

use serde::{Deserialize, Serialize};

use std::collections::HashMap;

use std::sync::{Arc, RwLock};

use tauri::{
    webview::PageLoadEvent, AppHandle, Listener, Manager, WebviewUrl, WebviewWindowBuilder,
};

use dashmap::DashMap;

use uuid::Uuid;

use std::time::Duration;
use tokio::sync::{oneshot, Notify};

use super::browser_error::BrowserError;
use reqwest;

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
    /// A thread-safe map to store oneshot senders for pending script executions, keyed by request ID.
    /// When a script result arrives, the sender is used to wake up the waiting receiver.
    result_waiters: Arc<DashMap<String, oneshot::Sender<String>>>,
    /// Map of session IDs to Notify objects for page load events
    page_load_waiters: Arc<DashMap<String, Arc<Notify>>>,
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
            result_waiters: Arc::new(DashMap::new()),
            page_load_waiters: Arc::new(DashMap::new()),
        }
    }

    /// Helper to apply platform-specific window settings (Linux focus fixes)
    fn apply_platform_window_settings(&self, _window: &tauri::WebviewWindow) {
        #[cfg(target_os = "linux")]
        {
            use log::error;
            if let Err(e) = _window.show() {
                error!("Failed to show window on Linux: {e}");
            }
            if let Err(e) = _window.set_focus() {
                error!("Failed to focus window on Linux: {e}");
            }
        }
    }

    /// Validates URL and returns normalized version.
    /// Supports: http://, https://
    ///
    /// # Arguments
    /// * `url` - The URL to validate
    ///
    /// # Returns
    /// A `Result` containing the normalized URL on success, or an error string on failure.
    fn validate_and_normalize_url(&self, url: &str) -> Result<String, String> {
        let parsed_result = url::Url::parse(url);

        match parsed_result {
            Ok(parsed) => {
                // Determine if we should allow based on scheme
                match parsed.scheme() {
                    "http" | "https" | "about" => Ok(url.to_string()),
                    scheme => Err(format!(
                        "Unsupported URL scheme '{}'. Allowed: http://, https://, about:",
                        scheme
                    )),
                }
            }
            Err(_) => {
                // Try prepending https://
                let with_proto = format!("https://{}", url);
                if let Ok(_parsed) = url::Url::parse(&with_proto) {
                    return Ok(with_proto);
                }

                Err(format!("Invalid URL format: {}", url))
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
    ) -> Result<(String, String), String> {
        // Validate URL first
        let validated_url = self.validate_and_normalize_url(url)?;

        let session_id = Uuid::new_v4().to_string();

        let window_label = format!("browser-{session_id}");

        // Check URL status (skip for about:blank)
        let parsed_url =
            url::Url::parse(&validated_url).map_err(|e| format!("Invalid URL format: {e}"))?;

        let status_check = if parsed_url.scheme() == "about" {
            None
        } else {
            Some(self.check_url_status(&validated_url).await)
        };

        let session_title = title.unwrap_or("Interactive Browser Agent");

        // Initialize page load waiter for this session BEFORE creating window to ensure we don't miss any events (though rare)
        let notify = Arc::new(Notify::new());
        self.page_load_waiters
            .insert(session_id.clone(), notify.clone());

        info!("Creating new browser session: {session_id} for URL: {url}");

        // Create WebviewWindow (independent browser window)

        let session_id_clone = session_id.clone();
        let page_load_waiters_clone = self.page_load_waiters.clone();

        let webview_window = WebviewWindowBuilder::new(
            &self.app_handle,
            &window_label,
            WebviewUrl::External(parsed_url),
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
        .initialization_script(format!(
            "window.__LIBR_AGENT_SESSION_ID__ = '{}';\n{}",
            session_id, INIT_SCRIPT
        ))
        .on_page_load(move |_window, payload| {
            if let PageLoadEvent::Finished = payload.event() {
                info!("Page loaded for session {}", session_id_clone);
                if let Some(notify) = page_load_waiters_clone.get(&session_id_clone) {
                    notify.notify_waiters();
                }
            }
        })
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
        };

        {
            let mut sessions = self
                .sessions
                .write()
                .map_err(|e| format!("Failed to acquire write lock: {e}"))?;

            sessions.insert(session_id.clone(), session);
        }

        // Register window event listeners

        // Clone additional state for cleanup in the event handler
        let sessions_clone = self.sessions.clone();
        let session_id_clone = session_id.clone();
        let page_load_waiters_clone = self.page_load_waiters.clone();

        webview_window.once("tauri://close-requested", move |_| {
            debug!("Browser window close requested for session: {session_id_clone}");

            // Full cleanup: Remove session from map
            if let Ok(mut sessions) = sessions_clone.write() {
                if sessions.remove(&session_id_clone).is_some() {
                    info!("Session {session_id_clone} removed (manual close)");
                }
            }

            // Cleanup pending result waiters (oneshot channels will auto-cleanup when dropped)
            // The timeout mechanism ensures max 30s lifetime for any pending request
            page_load_waiters_clone.remove(&session_id_clone);
        });

        info!("Browser session created successfully: {session_id}");

        // Check status result for the message
        let status_msg = match status_check {
            Some(Ok(status)) if status >= 400 => {
                warn!("URL {url} created session but returned status {status}");
                format!("Session created for {url} (HTTP {status})")
            }
            Some(Err(e)) => {
                warn!("URL {url} session created but check failed: {e}");
                format!("Session created for {url} (Network Error: {e})")
            }
            Some(Ok(_)) => {
                match tokio::time::timeout(Duration::from_secs(30), notify.notified()).await {
                    Ok(_) => {
                        info!("Initial page load completed for session {session_id}");
                        format!("Session created for {url} and page loaded")
                    }
                    Err(_) => {
                        info!("Initial page load timed out for session {session_id}");
                        format!("Session created for {url} (load wait timed out)")
                    }
                }
            }
            Option::None => {
                // This shouldn't happen since we always set status_check
                format!("Session created for {url}")
            }
        };

        Ok((session_id, status_msg))
    }

    /// Executes a given JavaScript snippet in a specific browser session's window.
    ///
    /// This method wraps the user-provided script in an async IIFE to handle promises
    /// and errors gracefully. It waits for the result using a oneshot channel pattern
    /// with a 30-second timeout. The result is delivered directly without polling.
    ///
    /// # Arguments
    /// * `session_id` - The ID of the session in which to execute the script.
    /// * `script` - The JavaScript code to execute.
    ///
    /// # Returns
    /// A `Result` containing the script execution result string, or an error string on failure.
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

            // Create oneshot channel BEFORE eval to prevent race condition
            let (tx, rx) = oneshot::channel::<String>();
            self.result_waiters.insert(request_id.clone(), tx);

            // Ensure window is focused on Linux before execution
            self.apply_platform_window_settings(&window);

            // Construct the full script to inject directly
            // This avoids using 'new Function' inside the browser context, bypassing CSP restrictions
            let execution_call = format!(
                r#"
(async function() {{
    // Check for runtime initialization
    if (!window.__LIBR_AGENT__) {{
        console.error('[LibrAgent] Runtime not initialized');
        // Try to send error via raw Tauri invoke if possible
        if (window.__TAURI__) {{
             try {{
                const payload = {{
                    sessionId: '{session_id}',
                    requestId: '{request_id}',
                    result: 'Error: Runtime not initialized (window.__LIBR_AGENT__ missing)'
                }};
                await window.__TAURI__.core.invoke('browser_script_result', {{ payload }});
             }} catch (e) {{ console.error(e); }}
        }}
        return;
    }}

    try {{
        // Wait for IPC first
        if (!await window.__LIBR_AGENT__.waitForIPC()) {{
            await window.__LIBR_AGENT__.sendResult('{session_id}', '{request_id}', 'Error: Tauri IPC failed to initialize', true);
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

            // Execute the script
            if let Err(e) = window.eval(&execution_call) {
                // Cleanup on eval failure
                self.result_waiters.remove(&request_id);
                error!("Failed to execute script wrapper in session {session_id}: {e}");
                return Err(format!("Failed to execute script: {e}"));
            }

            debug!("Script execution initiated, waiting for result: {request_id}");

            // Wait for result with timeout using oneshot channel
            match tokio::time::timeout(Duration::from_secs(30), rx).await {
                Ok(Ok(result)) => {
                    debug!("Script execution completed successfully: {request_id}");
                    Ok(result)
                }
                Ok(Err(_)) => {
                    // Channel was closed without sending (sender dropped)
                    self.result_waiters.remove(&request_id);
                    error!("Script result channel closed unexpectedly: {request_id}");
                    Err("Script execution failed: channel closed".to_string())
                }
                Err(_) => {
                    // Timeout occurred
                    self.result_waiters.remove(&request_id);
                    warn!("Script execution timeout after 30s: {request_id}");
                    Err(String::from(BrowserError::Timeout {
                        operation: "execute_script".to_string(),
                        duration_ms: 30000,
                        session_id: session_id.to_string(),
                    }))
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

        // Cleanup pending result waiters (oneshot channels will auto-cleanup when dropped)
        // The timeout mechanism ensures max 30s lifetime for any pending request
        self.page_load_waiters.remove(session_id);

        info!("Session {session_id} closed successfully");

        Ok("Session closed successfully".to_string())
    }

    /// Navigates a browser session to a new URL and waits for the page to load.
    pub async fn navigate_to_url(&self, session_id: &str, url: &str) -> Result<String, String> {
        // 1. Get session info (read lock)
        let (window_label, current_url) = {
            let sessions = self.sessions.read().map_err(|e| {
                String::from(BrowserError::LockFailed {
                    reason: format!("Failed to acquire read lock: {e}"),
                })
            })?;

            let session = sessions.get(session_id).ok_or_else(|| {
                String::from(BrowserError::SessionNotFound {
                    session_id: session_id.to_string(),
                })
            })?;
            (session.window_label.clone(), session.url.clone())
        };

        // 2. Resolve and Validate URL
        let target_url = match url::Url::parse(url) {
            Ok(parsed) => match parsed.scheme() {
                "http" | "https" | "about" => url.to_string(),
                scheme => {
                    return Err(format!(
                        "Unsupported URL scheme '{}'. Allowed: http://, https://, about:",
                        scheme
                    ))
                }
            },
            Err(_) => {
                // Try prepending https:// first (common user intent for "google.com")
                let with_proto = format!("https://{}", url);
                if url::Url::parse(&with_proto).is_ok() {
                    with_proto
                } else {
                    // Assume relative URL, try to resolve against current_url
                    let base = url::Url::parse(&current_url)
                        .map_err(|e| format!("Current session URL is invalid: {e}"))?;
                    let joined = base
                        .join(url)
                        .map_err(|e| format!("Failed to resolve relative URL: {e}"))?;
                    warn!("Detected relative URL '{}'. Resolved to '{}'", url, joined);
                    joined.to_string()
                }
            }
        };

        // 3. Check URL status
        let status_check = if target_url.starts_with("about:") {
            None
        } else {
            Some(self.check_url_status(&target_url).await)
        };

        info!("Navigating session {session_id} to {target_url}");

        if let Some(window) = self.app_handle.get_webview_window(&window_label) {
            // Prepare waiter
            let notify = self
                .page_load_waiters
                .entry(session_id.to_string())
                .or_insert_with(|| Arc::new(Notify::new()))
                .value()
                .clone();

            // Use eval to set window.location.href with proper JSON encoding to prevent injection
            let url_json = serde_json::to_string(&target_url)
                .map_err(|e| format!("Failed to encode URL: {e}"))?;
            let script = format!("window.location.href = {}", url_json);

            window
                .eval(&script)
                .map_err(|e| format!("Failed to navigate: {e}"))?;

            // Update session URL in the store
            {
                let mut sessions = self
                    .sessions
                    .write()
                    .map_err(|e| format!("Failed to acquire write lock: {e}"))?;
                if let Some(session) = sessions.get_mut(session_id) {
                    session.url = target_url.clone();
                }
            }

            // Handle return based on status_check
            match status_check {
                Some(Ok(status)) if status >= 400 => {
                    warn!("URL {target_url} returned status {status}, returning early");
                    return Ok(format!("Navigated to {target_url} (HTTP {status})"));
                }
                Some(Err(e)) => {
                    warn!("URL {target_url} check failed: {e}, returning early");
                    return Ok(format!("Navigated to {target_url} (Network Error: {e})"));
                }
                Option::None => {
                    // This shouldn't happen since we always set status_check
                    return Ok(format!("Navigated to {target_url}"));
                }
                Some(Ok(_)) => {} // HTTP/HTTPS success (200-399), proceed to wait for page load
            }

            // Wait for page load with timeout (HTTP/HTTPS only)
            info!("Waiting for page load in session {session_id}...");
            match tokio::time::timeout(Duration::from_secs(30), notify.notified()).await {
                Ok(_) => {
                    info!("Page load completed for session {session_id}");
                    Ok(format!("Navigated to {target_url} and page loaded"))
                }
                Err(_) => {
                    // Timeout
                    info!(
                        "Navigation timed out waiting for page load event in session {session_id}"
                    );
                    Ok(format!("Navigated to {target_url} (load wait timed out)"))
                }
            }
        } else {
            Err("Browser window not found".to_string())
        }
    }

    /// Checks the HTTP status of a URL using reqwest.
    async fn check_url_status(&self, url: &str) -> Result<u16, String> {
        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 LibrAgent Browser")
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| e.to_string())?;

        // We use GET instead of HEAD to be more robust against servers that block HEAD or return 405.
        // reqwest does not download the body unless we consume the stream, so it's efficient.
        let response = client.get(url).send().await.map_err(|e| e.to_string())?;
        Ok(response.status().as_u16())
    }

    /// Handles the page loaded notification from the frontend.
    pub fn handle_page_loaded(&self, session_id: &str) -> Result<(), String> {
        debug!("Received page load notification for session: {session_id}");
        if let Some(notify) = self.page_load_waiters.get(session_id) {
            notify.notify_one();
            Ok(())
        } else {
            // It's possible the session was closed or never properly initialized
            Err("Session waiter not found".to_string())
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

    /// Handles the script result received from the `browser_script_result` command.
    ///
    /// This method is called internally when the browser sends back the result of a
    /// script execution. It removes the oneshot sender from the waiters map and sends
    /// the result through the channel to wake up the waiting receiver.
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
        debug!("Received script result for session: {session_id}, request_id: {request_id}");

        // Remove sender from waiters and send result
        if let Some((_, sender)) = self.result_waiters.remove(&request_id) {
            // Send result through oneshot channel
            if sender.send(result).is_err() {
                // Receiver was dropped (timeout or cancelled)
                warn!(
                    "Failed to send script result for {request_id}: receiver dropped \
                     (likely timeout or cancelled request)"
                );
            } else {
                debug!("Script result delivered successfully: {request_id}");
            }
            Ok(())
        } else {
            // No waiter found - request may have timed out or was cancelled
            warn!(
                "No waiter found for script result: {request_id} \
                 (request may have timed out or been cancelled)"
            );
            Ok(()) // Not an error - just late arrival
        }
    }
}
