use chrono::{DateTime, Utc};
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use tauri::{AppHandle, Listener, Manager, WebviewUrl, WebviewWindowBuilder};
use tokio::sync::oneshot;
use tokio::time::{timeout, Duration};
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

#[derive(Debug)]
pub struct InteractiveBrowserServer {
    app_handle: AppHandle,
    sessions: Arc<RwLock<HashMap<String, BrowserSession>>>,
    pending_requests: Arc<Mutex<HashMap<String, oneshot::Sender<String>>>>,
}

impl Clone for InteractiveBrowserServer {
    fn clone(&self) -> Self {
        Self {
            app_handle: self.app_handle.clone(),
            sessions: self.sessions.clone(),
            pending_requests: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl InteractiveBrowserServer {
    pub fn new(app_handle: AppHandle) -> Self {
        Self {
            app_handle,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            pending_requests: Arc::new(Mutex::new(HashMap::new())),
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
            // Generate unique request ID for this script execution
            let request_id = Uuid::new_v4().to_string();
            let (sender, receiver) = oneshot::channel();

            // Store the sender in pending requests
            {
                let mut pending = self
                    .pending_requests
                    .lock()
                    .map_err(|e| format!("Failed to acquire pending requests lock: {}", e))?;
                pending.insert(request_id.clone(), sender);
            }

            // Use both DOMContentLoaded and immediate execution to handle all cases
            let wrapped_script = format!(
                r#"
                (function() {{
                    function executeScript() {{
                        try {{
                            const result = {};
                            window.__TAURI_INTERNALS__.invoke('send_content_from_webviewjs', {{
                                sessionId: '{}',
                                requestId: '{}',
                                content: typeof result === 'string' ? result : JSON.stringify(result)
                            }});
                        }} catch (error) {{
                            window.__TAURI_INTERNALS__.invoke('send_content_from_webviewjs', {{
                                sessionId: '{}',
                                requestId: '{}',
                                content: 'Error: ' + error.message
                            }});
                        }}
                    }}

                    // Execute immediately if DOM is already loaded, otherwise wait for DOMContentLoaded
                    if (document.readyState === 'loading') {{
                        window.addEventListener('DOMContentLoaded', executeScript);
                    }} else {{
                        executeScript();
                    }}
                }})();
                "#,
                script, session_id, request_id, session_id, request_id
            );

            window
                .eval(&wrapped_script)
                .map_err(|e| format!("Failed to inject script: {}", e))?;

            // Wait for the result with timeout (30 seconds)
            match timeout(Duration::from_secs(30), receiver).await {
                Ok(Ok(result)) => {
                    debug!("Script execution completed for session: {}", session_id);
                    Ok(result)
                }
                Ok(Err(_)) => {
                    error!("Script execution was cancelled for session: {}", session_id);
                    Err("Script execution was cancelled".to_string())
                }
                Err(_) => {
                    error!("Script execution timeout for session: {}", session_id);
                    // Clean up pending request on timeout
                    let mut pending = self
                        .pending_requests
                        .lock()
                        .map_err(|e| format!("Failed to acquire pending requests lock: {}", e))?;
                    pending.remove(&request_id);
                    Err("Script execution timeout (30 seconds)".to_string())
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

        // Use the new execute_script structure to get actual HTML content
        match self.execute_script(session_id, script).await {
            Ok(content) => {
                info!(
                    "Page content extracted for session {}: {} characters",
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

    /// Take a screenshot of the page using canvas API
    pub async fn take_screenshot(&self, session_id: &str) -> Result<String, String> {
        debug!("Taking screenshot info for session {}", session_id);

        let script = r#"
            (function() {
                try {
                    // 실제 이미지 캡처는 html2canvas 같은 외부 라이브러리 주입이 필요하므로,
                    // 이 단계에서는 페이지의 현재 상태 정보를 반환합니다.
                    return JSON.stringify({
                        type: 'screenshot_info',
                        viewport: {
                            width: window.innerWidth,
                            height: window.innerHeight,
                            devicePixelRatio: window.devicePixelRatio
                        },
                        url: window.location.href,
                        title: document.title,
                        timestamp: new Date().toISOString()
                    });
                } catch (error) {
                    return JSON.stringify({ error: 'Screenshot info failed: ' + error.message });
                }
            })()
        "#;

        match self.execute_script(session_id, script).await {
            Ok(result) => {
                info!("Screenshot info captured for session {}", session_id);
                Ok(result)
            }
            Err(e) => {
                error!(
                    "Failed to capture screenshot for session {}: {}",
                    session_id, e
                );
                Err(e)
            }
        }
    }

    /// Get page metadata (title, description, keywords, etc.)
    pub async fn get_page_metadata(&self, session_id: &str) -> Result<String, String> {
        debug!("Getting page metadata for session {}", session_id);

        let script = r#"
            (function() {
                const metadata = {
                    title: document.title,
                    description: '',
                    keywords: '',
                    author: '',
                    canonical: '',
                    ogTitle: '',
                    ogDescription: '',
                    ogImage: '',
                    viewport: '',
                    charset: document.characterSet,
                    lang: document.documentElement.lang,
                    url: window.location.href
                };

                // Get meta tags
                const metaTags = document.querySelectorAll('meta');
                metaTags.forEach(tag => {
                    const name = tag.getAttribute('name') || tag.getAttribute('property');
                    const content = tag.getAttribute('content');

                    if (name && content) {
                        switch(name.toLowerCase()) {
                            case 'description':
                                metadata.description = content;
                                break;
                            case 'keywords':
                                metadata.keywords = content;
                                break;
                            case 'author':
                                metadata.author = content;
                                break;
                            case 'viewport':
                                metadata.viewport = content;
                                break;
                            case 'og:title':
                                metadata.ogTitle = content;
                                break;
                            case 'og:description':
                                metadata.ogDescription = content;
                                break;
                            case 'og:image':
                                metadata.ogImage = content;
                                break;
                        }
                    }
                });

                // Get canonical URL
                const canonical = document.querySelector('link[rel="canonical"]');
                if (canonical) {
                    metadata.canonical = canonical.href;
                }

                return JSON.stringify(metadata, null, 2);
            })()
        "#;

        match self.execute_script(session_id, script).await {
            Ok(result) => {
                info!("Page metadata extracted for session {}", session_id);
                Ok(result)
            }
            Err(e) => {
                error!(
                    "Failed to get page metadata for session {}: {}",
                    session_id, e
                );
                Err(e)
            }
        }
    }

    /// Get all links from the page
    pub async fn get_page_links(&self, session_id: &str) -> Result<String, String> {
        debug!("Getting page links for session {}", session_id);
        let script = r#"
            (function() {
                const links = Array.from(document.querySelectorAll('a[href]')).map(a => ({
                    href: a.href,
                    text: a.textContent.trim()
                }));
                return JSON.stringify({ count: links.length, links: links });
            })()
        "#;

        match self.execute_script(session_id, script).await {
            Ok(result) => {
                info!("Page links extracted for session {}", session_id);
                Ok(result)
            }
            Err(e) => {
                error!("Failed to get page links for session {}: {}", session_id, e);
                Err(e)
            }
        }
    }

    /// Get all images from the page
    pub async fn get_page_images(&self, session_id: &str) -> Result<String, String> {
        debug!("Getting page images for session {}", session_id);
        let script = r#"
            (function() {
                const images = Array.from(document.querySelectorAll('img')).map(img => ({
                    src: img.src,
                    alt: img.alt,
                    width: img.naturalWidth,
                    height: img.naturalHeight
                }));

                // Also get background images from CSS
                const elementsWithBgImages = [];
                document.querySelectorAll('*').forEach((el, index) => {
                    const bgImage = window.getComputedStyle(el).backgroundImage;
                    if (bgImage && bgImage !== 'none' && bgImage.includes('url(')) {
                        const url = bgImage.match(/url\(['"]?(.*?)['"]?\)/);
                        if (url && url[1]) {
                            elementsWithBgImages.push({
                                element: el.tagName.toLowerCase(),
                                backgroundImage: url[1]
                            });
                        }
                    }
                });

                return JSON.stringify({
                    count: images.length,
                    images: images,
                    totalBackgroundImages: elementsWithBgImages.length,
                    backgroundImages: elementsWithBgImages
                });
            })()
        "#;

        match self.execute_script(session_id, script).await {
            Ok(result) => {
                info!("Page images extracted for session {}", session_id);
                Ok(result)
            }
            Err(e) => {
                error!(
                    "Failed to get page images for session {}: {}",
                    session_id, e
                );
                Err(e)
            }
        }
    }

    /// Get page performance metrics
    pub async fn get_page_performance(&self, session_id: &str) -> Result<String, String> {
        debug!(
            "Getting page performance metrics for session {}",
            session_id
        );

        let script = r#"
            (function() {
                try {
                    const performance = window.performance;
                    if (!performance) {
                        return JSON.stringify({ error: 'Performance API not supported.' });
                    }
                    const navigation = performance.getEntriesByType('navigation')[0];
                    const paint = performance.getEntriesByType('paint');
                    const metrics = {
                        loadTime: navigation ? navigation.duration : null,
                        domContentLoadedTime: navigation ? navigation.domContentLoadedEventEnd : null,
                        firstContentfulPaint: paint.find(p => p.name === 'first-contentful-paint')?.startTime || null,
                        timestamp: Date.now(),
                        url: window.location.href,
                        navigation: navigation ? {
                            domContentLoaded: navigation.domContentLoadedEventEnd - navigation.domContentLoadedEventStart,
                            loadComplete: navigation.loadEventEnd - navigation.loadEventStart,
                            domComplete: navigation.domComplete - navigation.navigationStart,
                            responseStart: navigation.responseStart - navigation.navigationStart,
                            responseEnd: navigation.responseEnd - navigation.navigationStart
                        } : null,
                        paint: {},
                        memory: performance.memory ? {
                            usedJSHeapSize: performance.memory.usedJSHeapSize,
                            totalJSHeapSize: performance.memory.totalJSHeapSize,
                            jsHeapSizeLimit: performance.memory.jsHeapSizeLimit
                        } : null,
                        connection: navigator.connection ? {
                            effectiveType: navigator.connection.effectiveType,
                            downlink: navigator.connection.downlink,
                            rtt: navigator.connection.rtt
                        } : null
                    };

                    paint.forEach(entry => {
                        metrics.paint[entry.name] = entry.startTime;
                    });

                    return JSON.stringify(metrics, null, 2);
                } catch (error) {
                    return JSON.stringify({ error: 'Failed to get performance metrics: ' + error.message });
                }
            })()
        "#;

        match self.execute_script(session_id, script).await {
            Ok(result) => {
                info!(
                    "Page performance metrics extracted for session {}",
                    session_id
                );
                Ok(result)
            }
            Err(e) => {
                error!(
                    "Failed to get page performance for session {}: {}",
                    session_id, e
                );
                Err(e)
            }
        }
    }

    /// Handle content received from WebView JavaScript
    pub async fn handle_received_content(
        &self,
        session_id: String,
        request_id: String,
        content: String,
    ) -> Result<(), String> {
        log::info!(
            "Received content from session {} (request {}): {} characters",
            session_id,
            request_id,
            content.len()
        );

        // Validate session exists
        {
            let sessions = self
                .sessions
                .read()
                .map_err(|e| format!("Failed to acquire read lock: {}", e))?;
            if !sessions.contains_key(&session_id) {
                return Err("Session not found".to_string());
            }
        }

        // Find and wake up the pending request
        let sender = {
            let mut pending = self
                .pending_requests
                .lock()
                .map_err(|e| format!("Failed to acquire pending requests lock: {}", e))?;
            pending.remove(&request_id)
        };

        if let Some(sender) = sender {
            // Send the result to the waiting execute_script call
            if let Err(_) = sender.send(content.clone()) {
                log::warn!("Failed to send result to waiting request: {}", request_id);
            } else {
                log::debug!("Successfully delivered result to request: {}", request_id);
            }
        } else {
            log::warn!("No pending request found for ID: {}", request_id);
        }

        // Log content preview for debugging
        log::debug!(
            "Content preview: {}",
            if content.len() > 200 {
                format!("{}...", &content[..200])
            } else {
                content.clone()
            }
        );

        // TODO: Add file saving, data processing, etc.
        // Example:
        // - Save HTML content to files
        // - Process extracted data
        // - Trigger webhooks or notifications
        // - Store in database

        Ok(())
    }
}
