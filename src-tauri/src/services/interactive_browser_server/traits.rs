/// Trait defining the environment adapter for the InteractiveBrowserServer.
/// This decouples the domain logic from the underlying framework (e.g., Tauri).
pub trait BrowserEnvironment: Send + Sync {
    /// Creates a new browser window/session.
    ///
    /// # Arguments
    /// * `session_id` - The unique identifier for the session.
    /// * `window_label` - The label used to identify the window.
    /// * `url` - The initial URL to load.
    /// * `title` - The window title.
    /// * `visible` - Whether the window should be visible.
    /// * `on_page_load` - A callback to execute when the page is loaded.
    /// * `on_close` - A callback to execute when the window is closed.
    fn create_browser_window(
        &self,
        session_id: &str,
        window_label: &str,
        url: &str,
        title: &str,
        visible: bool,
        on_page_load: Box<dyn Fn() + Send + Sync>,
        on_close: Box<dyn Fn() + Send + Sync>,
    ) -> Result<(), String>;

    /// Closes an existing browser window/session.
    fn close_browser_window(&self, window_label: &str) -> Result<(), String>;

    /// Executes a script in the browser window and optionally waits for an IPC result.
    /// (Note: The `eval` simply runs the string; the actual result is returned via IPC.)
    fn execute_script(&self, window_label: &str, script: &str) -> Result<(), String>;
}
