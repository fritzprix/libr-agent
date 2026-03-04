/// Parameters for creating a new browser window.
pub struct CreateWindowParams<'a> {
    pub session_id: &'a str,
    pub window_label: &'a str,
    pub url: &'a str,
    pub title: &'a str,
    pub visible: bool,
    pub on_page_load: Box<dyn Fn() + Send + Sync>,
    pub on_close: Box<dyn Fn() + Send + Sync>,
}

/// Trait defining the environment adapter for the InteractiveBrowserServer.
/// This decouples the domain logic from the underlying framework (e.g., Tauri).
pub trait BrowserEnvironment: Send + Sync {
    /// Creates a new browser window/session.
    fn create_browser_window(&self, params: CreateWindowParams<'_>) -> Result<(), String>;

    /// Closes an existing browser window/session.
    fn close_browser_window(&self, window_label: &str) -> Result<(), String>;

    /// Executes a script in the browser window and optionally waits for an IPC result.
    /// (Note: The `eval` simply runs the string; the actual result is returned via IPC.)
    fn execute_script(&self, window_label: &str, script: &str) -> Result<(), String>;
}
