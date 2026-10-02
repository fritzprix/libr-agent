//! Browser profile discovery and import into LibrAgent app-local storage.
//!
//! Supports Chromium-family User Data copies (Chrome, Edge, Brave, …). Agents never see
//! filesystem paths — they request the imported profile via
//! `browser__createSession({ use_profile: true })` after explicit user confirmation.
//!
//! Legacy Firefox cookie-inject profiles may still exist on disk; new imports are Chromium-only.

mod chrome_attach;
mod discover;
mod firefox;
mod import;
mod registry;

pub use chrome_attach::{
    chrome_profile_appears_in_use, open_imported_profile_for_signin, pick_loopback_debug_port,
    quit_processes_holding_imported_profile, spawn_system_chrome_for_profile,
    spawn_system_chrome_for_profile_async, wait_for_cdp_ready, ChromeProfileSpawnOptions,
    QuitSavedLoginWindowsReport,
};
pub use discover::{discover_browser_profiles, discover_chrome_profiles, DiscoveredBrowserProfile};
pub use firefox::{read_inject_cookies_file, InjectableCookie, INJECT_COOKIES_FILE};
pub use import::{
    delete_imported_browser_profile, import_browser_profiles, import_chrome_profiles,
    list_discoverable_browser_profiles, list_running_browsers_for_import,
    list_running_browsers_for_profiles, quit_browsers_for_profile_import,
    set_default_browser_profile, DiscoverableBrowserProfile, ImportReport, QuitBrowsersReport,
};
pub use registry::{
    ensure_under_profiles_storage, has_any_imported_profile, list_imported_profiles, load_registry,
    resolve_default_imported_user_data_dir, set_default_imported_profile, BrowserProfileInfo,
    BrowserProfileRegistry, ImportKind, ImportedProfile,
};
