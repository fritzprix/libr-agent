mod client;
mod contracts;
mod page;
mod runtime;
mod server;

pub use client::BrowserAutomationClient;
pub use contracts::{ConsoleEntry, HistoryNavigationStatus, PageClassification, PageState};
pub use page::{
    classify_browser_page, serialize_browser_result_value, validate_full_page_dimensions,
    validate_screenshot_png_bytes, MAX_FULL_PAGE_PIXELS, MAX_SCREENSHOT_BYTES,
};
pub use runtime::{
    browser_runtime_cache_root, browser_runtime_profile_dir, browser_runtime_profile_root,
};
pub use server::run_sidecar_mode;

pub const BROWSER_SIDECAR_FLAG: &str = "--browser-sidecar";
