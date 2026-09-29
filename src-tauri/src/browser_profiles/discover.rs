use std::path::{Path, PathBuf};

/// How an imported profile is applied at automation time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileEngine {
    /// Full Chromium User Data tree (`--user-data-dir`).
    ChromiumUserData,
    /// Legacy Firefox cookie inject — no longer discovered or newly imported.
    FirefoxCookies,
}

/// A browser profile detected on the host (source paths only; never shown to agents).
#[derive(Debug, Clone)]
pub struct DiscoveredBrowserProfile {
    /// Stable slug used as registry key (`chrome_default`, `edge_default`, …).
    pub name: String,
    /// Human-readable label (`Chrome Default`, `Edge Default`, …).
    pub label: String,
    /// Stable browser id (`chrome`, `edge`, `brave`, …).
    pub browser_id: String,
    pub engine: ProfileEngine,
    /// Absolute path to the Chromium profile directory (e.g. `…/Default`).
    pub profile_dir: PathBuf,
    /// Chromium User Data root (`--user-data-dir`).
    pub user_data_root: PathBuf,
}

/// Discover installed Chromium-family Default profiles (Chrome / Edge / Brave / …).
///
/// Firefox is intentionally unsupported: automation runs Chromium, and cookie-only
/// inject cannot reliably preserve Google sessions.
pub fn discover_chrome_profiles() -> Vec<DiscoveredBrowserProfile> {
    discover_browser_profiles()
}

pub fn discover_browser_profiles() -> Vec<DiscoveredBrowserProfile> {
    let mut profiles = Vec::new();
    for candidate in chromium_user_data_candidates() {
        if !candidate.root.is_dir() {
            continue;
        }
        profiles.extend(scan_chromium_user_data_root(
            &candidate.root,
            candidate.browser_id,
            candidate.display_name,
        ));
    }

    // Prefer earlier candidates (native before Snap/Flatpak) when the same slug appears twice.
    let mut seen = std::collections::HashSet::new();
    profiles.retain(|profile| seen.insert(profile.name.clone()));
    profiles.sort_by(|a, b| a.name.cmp(&b.name));
    profiles
}

struct ChromiumRootCandidate {
    browser_id: &'static str,
    display_name: &'static str,
    root: PathBuf,
}

fn chromium_user_data_candidates() -> Vec<ChromiumRootCandidate> {
    let mut roots = Vec::new();

    match std::env::consts::OS {
        "windows" => {
            if let Ok(local) = std::env::var("LOCALAPPDATA") {
                let local = PathBuf::from(local);
                push_chromium(
                    &mut roots,
                    "chrome",
                    "Chrome",
                    local.join(r"Google\Chrome\User Data"),
                );
                push_chromium(
                    &mut roots,
                    "chrome_beta",
                    "Chrome Beta",
                    local.join(r"Google\Chrome Beta\User Data"),
                );
                push_chromium(
                    &mut roots,
                    "chrome_canary",
                    "Chrome Canary",
                    local.join(r"Google\Chrome SxS\User Data"),
                );
                push_chromium(
                    &mut roots,
                    "edge",
                    "Edge",
                    local.join(r"Microsoft\Edge\User Data"),
                );
                push_chromium(
                    &mut roots,
                    "edge_beta",
                    "Edge Beta",
                    local.join(r"Microsoft\Edge Beta\User Data"),
                );
                push_chromium(
                    &mut roots,
                    "edge_dev",
                    "Edge Dev",
                    local.join(r"Microsoft\Edge Dev\User Data"),
                );
                push_chromium(
                    &mut roots,
                    "brave",
                    "Brave",
                    local.join(r"BraveSoftware\Brave-Browser\User Data"),
                );
                push_chromium(
                    &mut roots,
                    "chromium",
                    "Chromium",
                    local.join(r"Chromium\User Data"),
                );
                push_chromium(
                    &mut roots,
                    "vivaldi",
                    "Vivaldi",
                    local.join(r"Vivaldi\User Data"),
                );
            }
        }
        "macos" => {
            if let Some(home) = dirs::home_dir() {
                let support = home.join("Library/Application Support");
                push_chromium(
                    &mut roots,
                    "chrome",
                    "Chrome",
                    support.join("Google/Chrome"),
                );
                push_chromium(
                    &mut roots,
                    "chrome_beta",
                    "Chrome Beta",
                    support.join("Google/Chrome Beta"),
                );
                push_chromium(
                    &mut roots,
                    "chrome_canary",
                    "Chrome Canary",
                    support.join("Google/Chrome Canary"),
                );
                push_chromium(&mut roots, "edge", "Edge", support.join("Microsoft Edge"));
                push_chromium(
                    &mut roots,
                    "edge_beta",
                    "Edge Beta",
                    support.join("Microsoft Edge Beta"),
                );
                push_chromium(
                    &mut roots,
                    "edge_dev",
                    "Edge Dev",
                    support.join("Microsoft Edge Dev"),
                );
                push_chromium(
                    &mut roots,
                    "brave",
                    "Brave",
                    support.join("BraveSoftware/Brave-Browser"),
                );
                push_chromium(&mut roots, "chromium", "Chromium", support.join("Chromium"));
                push_chromium(&mut roots, "vivaldi", "Vivaldi", support.join("Vivaldi"));
            }
        }
        "linux" => {
            if let Some(home) = dirs::home_dir() {
                let config = home.join(".config");
                push_chromium(&mut roots, "chrome", "Chrome", config.join("google-chrome"));
                push_chromium(
                    &mut roots,
                    "chrome_beta",
                    "Chrome Beta",
                    config.join("google-chrome-beta"),
                );
                push_chromium(
                    &mut roots,
                    "chrome_canary",
                    "Chrome Unstable",
                    config.join("google-chrome-unstable"),
                );
                push_chromium(&mut roots, "chromium", "Chromium", config.join("chromium"));
                push_chromium(&mut roots, "edge", "Edge", config.join("microsoft-edge"));
                push_chromium(
                    &mut roots,
                    "edge_beta",
                    "Edge Beta",
                    config.join("microsoft-edge-beta"),
                );
                push_chromium(
                    &mut roots,
                    "edge_dev",
                    "Edge Dev",
                    config.join("microsoft-edge-dev"),
                );
                push_chromium(
                    &mut roots,
                    "brave",
                    "Brave",
                    config.join("BraveSoftware/Brave-Browser"),
                );
                push_chromium(&mut roots, "vivaldi", "Vivaldi", config.join("vivaldi"));
                // Snap / Flatpak common locations
                push_chromium(
                    &mut roots,
                    "chromium",
                    "Chromium (Snap)",
                    home.join("snap/chromium/common/chromium"),
                );
                push_chromium(
                    &mut roots,
                    "chrome",
                    "Chrome (Flatpak)",
                    home.join(".var/app/com.google.Chrome/config/google-chrome"),
                );
                push_chromium(
                    &mut roots,
                    "chromium",
                    "Chromium (Flatpak)",
                    home.join(".var/app/org.chromium.Chromium/config/chromium"),
                );
                push_chromium(
                    &mut roots,
                    "brave",
                    "Brave (Flatpak)",
                    home.join(".var/app/com.brave.Browser/config/BraveSoftware/Brave-Browser"),
                );
                push_chromium(
                    &mut roots,
                    "edge",
                    "Edge (Flatpak)",
                    home.join(".var/app/com.microsoft.Edge/config/microsoft-edge"),
                );
            }
        }
        _ => {}
    }

    roots
}

fn push_chromium(
    roots: &mut Vec<ChromiumRootCandidate>,
    browser_id: &'static str,
    display_name: &'static str,
    root: PathBuf,
) {
    roots.push(ChromiumRootCandidate {
        browser_id,
        display_name,
        root,
    });
}

/// Lower number = preferred registry default when multiple browsers are imported.
pub fn browser_default_priority(browser_id: &str) -> u8 {
    match browser_id {
        "chrome" => 0,
        "edge" => 1,
        "brave" => 2,
        "chromium" => 3,
        "vivaldi" => 4,
        "chrome_beta" | "edge_beta" => 5,
        "chrome_canary" | "edge_dev" => 6,
        // Legacy registry entries only — Firefox is no longer imported.
        "firefox" => 20,
        _ => 15,
    }
}

/// End-user label for a browser id (matches process detection labels).
pub fn friendly_browser_label(browser_id: &str) -> &'static str {
    match browser_id {
        "chrome" | "chrome_beta" | "chrome_canary" => "Chrome",
        "edge" | "edge_beta" | "edge_dev" => "Edge",
        "brave" => "Brave",
        "chromium" => "Chromium",
        "vivaldi" => "Vivaldi",
        "firefox" => "Firefox",
        _ => "Browser",
    }
}

fn scan_chromium_user_data_root(
    user_data_root: &Path,
    browser_id: &str,
    display_name: &str,
) -> Vec<DiscoveredBrowserProfile> {
    let Ok(entries) = std::fs::read_dir(user_data_root) else {
        return Vec::new();
    };

    let mut profiles = Vec::new();
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() {
            continue;
        }
        let name = entry.file_name();
        let Some(dir_name) = name.to_str() else {
            continue;
        };
        let Some((slug, label)) = classify_profile_dir_name(browser_id, display_name, dir_name)
        else {
            continue;
        };
        let profile_dir = entry.path();
        if !looks_like_chrome_profile(&profile_dir) {
            continue;
        }
        profiles.push(DiscoveredBrowserProfile {
            name: slug,
            label,
            browser_id: browser_id.to_string(),
            engine: ProfileEngine::ChromiumUserData,
            profile_dir,
            user_data_root: user_data_root.to_path_buf(),
        });
    }

    profiles
}

/// Classify a Chromium profile folder name, namespaced by browser to avoid collisions.
fn classify_profile_dir_name(
    browser_id: &str,
    display_name: &str,
    dir_name: &str,
) -> Option<(String, String)> {
    if dir_name.eq_ignore_ascii_case("Default") {
        return Some((
            format!("{browser_id}_default"),
            format!("{display_name} Default"),
        ));
    }
    // Skip Guest — rarely useful for agent login reuse.
    if dir_name.eq_ignore_ascii_case("Guest Profile") {
        return None;
    }
    let rest = dir_name.strip_prefix("Profile ")?;
    if rest.is_empty() || !rest.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some((
        format!("{browser_id}_profile_{rest}"),
        format!("{display_name} Profile {rest}"),
    ))
}

fn looks_like_chrome_profile(profile_dir: &Path) -> bool {
    profile_dir.join("Preferences").is_file()
        || profile_dir.join("Cookies").is_file()
        || profile_dir.join("Network").is_dir()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_namespaced_profile_names() {
        assert_eq!(
            classify_profile_dir_name("chrome", "Chrome", "Default"),
            Some(("chrome_default".into(), "Chrome Default".into()))
        );
        assert_eq!(
            classify_profile_dir_name("edge", "Edge", "Profile 2"),
            Some(("edge_profile_2".into(), "Edge Profile 2".into()))
        );
        assert_eq!(
            classify_profile_dir_name("brave", "Brave", "Default"),
            Some(("brave_default".into(), "Brave Default".into()))
        );
        assert_eq!(
            classify_profile_dir_name("chrome", "Chrome", "Guest Profile"),
            None
        );
        assert_eq!(
            classify_profile_dir_name("chrome", "Chrome", "System Profile"),
            None
        );
    }

    #[test]
    fn chrome_and_edge_defaults_do_not_collide() {
        let chrome = classify_profile_dir_name("chrome", "Chrome", "Default").unwrap();
        let edge = classify_profile_dir_name("edge", "Edge", "Default").unwrap();
        assert_ne!(chrome.0, edge.0);
    }

    #[test]
    fn chrome_priority_beats_other_chromium_family() {
        assert!(browser_default_priority("chrome") < browser_default_priority("edge"));
        assert!(browser_default_priority("edge") < browser_default_priority("brave"));
        assert!(browser_default_priority("brave") < browser_default_priority("chromium"));
    }
}
