mod client;
mod contracts;
mod page;
mod runtime;
mod server;

pub use client::BrowserAutomationClient;
pub use contracts::{HistoryNavigationStatus, PageClassification, PageState};
pub use page::{classify_browser_page, serialize_browser_result_value};
pub use runtime::{
    browser_runtime_cache_root, browser_runtime_profile_dir, browser_runtime_profile_root,
};
pub use server::run_sidecar_mode;

pub const BROWSER_SIDECAR_FLAG: &str = "--browser-sidecar";
