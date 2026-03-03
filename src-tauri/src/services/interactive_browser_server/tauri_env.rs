use std::sync::Arc;

use log::info;
use tauri::{AppHandle, Listener, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri::webview::PageLoadEvent;

use super::constants::INIT_SCRIPT;
use super::traits::BrowserEnvironment;

/// An implementation of `BrowserEnvironment` specifically for the Tauri framework.
pub struct TauriBrowserEnvironment {
    app_handle: AppHandle,
}

impl TauriBrowserEnvironment {
    pub fn new(app_handle: AppHandle) -> Self {
        Self { app_handle }
    }

    /// Helper to apply platform-specific window settings (Linux focus fixes)
    fn apply_platform_window_settings(&self, _window: &tauri::WebviewWindow) {
        // No platform-specific settings needed anymore
    }
}

impl BrowserEnvironment for TauriBrowserEnvironment {
    fn create_browser_window(
        &self,
        session_id: &str,
        window_label: &str,
        url: &str,
        title: &str,
        visible: bool,
        on_page_load: Box<dyn Fn() + Send + Sync>,
        on_close: Box<dyn Fn() + Send + Sync>,
    ) -> Result<(), String> {
        let parsed_url = url::Url::parse(url).map_err(|e| format!("Invalid URL format: {e}"))?;

        let session_id_clone = session_id.to_string();

        let webview_window = WebviewWindowBuilder::new(
            &self.app_handle,
            window_label,
            WebviewUrl::External(parsed_url),
        )
        .title(format!("{} - {}", title, session_id.to_uppercase()))
        .inner_size(1200.0, 800.0)
        .resizable(true)
        .maximizable(true)
        .minimizable(true)
        .center()
        .focused(visible)
        .visible(visible)
        .devtools(cfg!(debug_assertions))
        .initialization_script(format!(
            "window.__LIBR_AGENT_SESSION_ID__ = '{}';\n{}",
            session_id, INIT_SCRIPT
        ))
        .on_page_load(move |_window, payload| {
            if let PageLoadEvent::Finished = payload.event() {
                info!("Page loaded for session {}", session_id_clone);
                on_page_load();
            }
        })
        .accept_first_mouse(true)
        .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 LibrAgent Browser")
        .build()
        .map_err(|e| format!("Failed to create browser window: {e}"))?;

        // Apply platform-specific settings (Linux focus, etc.)
        self.apply_platform_window_settings(&webview_window);

        webview_window.once("tauri://close-requested", move |_| {
            on_close();
        });

        Ok(())
    }

    fn close_browser_window(&self, window_label: &str) -> Result<(), String> {
        if let Some(window) = self.app_handle.get_webview_window(window_label) {
            window
                .close()
                .map_err(|e| format!("Failed to close window: {e}"))?;
        }
        Ok(())
    }

    fn execute_script(&self, window_label: &str, script: &str) -> Result<(), String> {
        if let Some(window) = self.app_handle.get_webview_window(window_label) {
            self.apply_platform_window_settings(&window);
            window
                .eval(script)
                .map_err(|e| format!("Failed to execute script: {e}"))
        } else {
            Err("Browser window not found".to_string())
        }
    }
}
