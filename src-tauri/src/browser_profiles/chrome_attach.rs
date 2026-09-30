//! Spawn system Chrome/Edge against an app-local User Data dir (no chromiumoxide launch flags).
//!
//! Google rejects CDP-driven login ("This browser or app may not be secure"). The supported
//! path is: open a normal Chrome window on the LibrAgent profile for manual sign-in, then
//! later attach via `Browser::connect` for agent sessions.

use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use log::{info, warn};

use super::registry::{ensure_under_profiles_storage, load_registry, ImportKind};

const PROFILE_IN_USE_HINT: &str = "Close the LibrAgent Chrome window from Settings → Open to sign in (or any other Chrome using this saved login), then retry.";

/// Pick an unused TCP port on 127.0.0.1 for Chrome remote debugging.
pub fn pick_loopback_debug_port() -> Result<u16, String> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|e| format!("Failed to allocate loopback debug port: {e}"))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("Failed to read loopback debug port: {e}"))?
        .port();
    drop(listener);
    Ok(port)
}

/// Prefer a real installed Chrome/Edge/Brave binary (not distro Chromium or bundled CfT).
pub(crate) fn resolve_system_chrome_executable() -> Result<PathBuf, String> {
    if let Ok(path) = std::env::var("LIBRAGENT_BROWSER_EXECUTABLE") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Ok(path);
        }
        return Err(format!(
            "LIBRAGENT_BROWSER_EXECUTABLE points to a missing browser executable: {}",
            path.display()
        ));
    }

    let candidates = system_chrome_candidate_paths();
    for path in candidates {
        if path.is_file() {
            return Ok(path);
        }
    }

    Err(
        "No system Chrome, Edge, or Brave found. Install Google Chrome or Microsoft Edge to use saved browser logins (bundled Chromium cannot sign into Google)."
            .to_string(),
    )
}

fn system_chrome_candidate_paths() -> Vec<PathBuf> {
    let mut out = Vec::new();

    #[cfg(target_os = "macos")]
    {
        out.push(PathBuf::from(
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        ));
        out.push(PathBuf::from(
            "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
        ));
        out.push(PathBuf::from(
            "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser",
        ));
    }

    #[cfg(target_os = "linux")]
    {
        // Prefer Chrome/Edge/Brave for Google sign-in. Distro Chromium often fails that flow.
        for name in [
            "google-chrome-stable",
            "google-chrome",
            "microsoft-edge-stable",
            "microsoft-edge",
            "brave-browser",
        ] {
            if let Ok(path) = which_program(name) {
                out.push(path);
            }
        }
    }

    #[cfg(windows)]
    {
        let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
        let program_files = std::env::var_os("PROGRAMFILES").map(PathBuf::from);
        let program_files_x86 = std::env::var_os("PROGRAMFILES(X86)").map(PathBuf::from);
        for root in [local, program_files, program_files_x86]
            .into_iter()
            .flatten()
        {
            out.push(root.join("Google/Chrome/Application/chrome.exe"));
            out.push(root.join("Microsoft/Edge/Application/msedge.exe"));
            out.push(root.join("BraveSoftware/Brave-Browser/Application/brave.exe"));
        }
    }

    out
}

#[cfg(target_os = "linux")]
fn which_program(name: &str) -> Result<PathBuf, ()> {
    let output = Command::new("which").arg(name).output().map_err(|_| ())?;
    if !output.status.success() {
        return Err(());
    }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() {
        return Err(());
    }
    Ok(PathBuf::from(path))
}

/// True when Chrome's profile SingletonLock points at a still-running process.
pub fn chrome_profile_appears_in_use(user_data_dir: &Path) -> bool {
    let lock_path = user_data_dir.join("SingletonLock");
    // Chrome's SingletonLock is often a symlink to "hostname-pid" (not a real path).
    // `Path::exists` follows links and would report false — use symlink_metadata instead.
    let Ok(meta) = std::fs::symlink_metadata(&lock_path) else {
        return false;
    };

    #[cfg(unix)]
    {
        if meta.file_type().is_symlink() {
            if let Ok(target) = std::fs::read_link(&lock_path) {
                if let Some(pid) = parse_chrome_singleton_pid(&target) {
                    return process_appears_alive(pid);
                }
            }
        }
        // Non-symlink lock (or unreadable target): treat as in-use to avoid a confusing CDP timeout.
        true
    }

    #[cfg(windows)]
    {
        let _ = meta;
        // Windows holds SingletonLock while Chrome is running; a live second instance fails fast.
        true
    }
}

#[cfg(any(unix, test))]
fn parse_chrome_singleton_pid(target: impl AsRef<std::ffi::OsStr>) -> Option<u32> {
    let raw = target.as_ref().to_string_lossy();
    // Chrome writes "hostname-pid" (symlink target on Unix).
    let pid_str = raw.rsplit('-').next()?;
    pid_str.parse().ok()
}

#[cfg(target_os = "linux")]
fn process_appears_alive(pid: u32) -> bool {
    Path::new(&format!("/proc/{pid}")).exists()
}

#[cfg(target_os = "macos")]
fn process_appears_alive(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn profile_in_use_error() -> String {
    format!(
        "This saved browser login is already open in another Chrome window. {PROFILE_IN_USE_HINT}"
    )
}

fn attach_failed_before_cdp_error(user_data_dir: &Path) -> String {
    if chrome_profile_appears_in_use(user_data_dir) {
        return profile_in_use_error();
    }
    format!("Chrome exited before remote debugging was ready. {PROFILE_IN_USE_HINT}")
}

/// Options for spawning Chrome against an app-local profile.
#[derive(Debug, Clone)]
pub struct ChromeProfileSpawnOptions {
    pub user_data_dir: PathBuf,
    /// When set, Chrome listens for CDP on `127.0.0.1:{port}` only.
    pub debug_port: Option<u16>,
    pub headless: bool,
    /// Initial URL (e.g. Google accounts for Settings sign-in).
    pub start_url: Option<String>,
}

/// Spawn system Chrome/Edge with a LibrAgent-owned `--user-data-dir`.
///
/// Does **not** use chromiumoxide `Browser::launch` / DEFAULT_ARGS (those trip Google login).
pub fn spawn_system_chrome_for_profile(
    options: &ChromeProfileSpawnOptions,
) -> Result<std::process::Child, String> {
    let mut cmd = build_system_chrome_command(options)?;
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
        const DETACHED_PROCESS: u32 = 0x00000008;
        // Detach so Settings sign-in survives if the parent waits elsewhere.
        if options.debug_port.is_none() {
            cmd.creation_flags(CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS);
        }
    }

    info!(
        "Spawning system browser for profile {} (debug_port={:?}, headless={})",
        options.user_data_dir.display(),
        options.debug_port,
        options.headless
    );

    cmd.spawn()
        .map_err(|e| format!("Failed to launch system Chrome/Edge for profile sign-in: {e}"))
}

/// Async spawn for sidecar attach (owned child + CDP connect).
pub async fn spawn_system_chrome_for_profile_async(
    options: &ChromeProfileSpawnOptions,
) -> Result<tokio::process::Child, String> {
    let std_cmd = build_system_chrome_command(options)?;
    let mut cmd = tokio::process::Command::from(std_cmd);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);

    info!(
        "Spawning system browser (async) for profile {} (debug_port={:?}, headless={})",
        options.user_data_dir.display(),
        options.debug_port,
        options.headless
    );

    cmd.spawn()
        .map_err(|e| format!("Failed to launch system Chrome/Edge for profile attach: {e}"))
}

fn build_system_chrome_command(options: &ChromeProfileSpawnOptions) -> Result<Command, String> {
    let user_data_dir = ensure_under_profiles_storage(&options.user_data_dir)?;
    if !user_data_dir.is_dir() {
        std::fs::create_dir_all(&user_data_dir)
            .map_err(|e| format!("Failed to create browser profile directory: {e}"))?;
    }

    let executable = resolve_system_chrome_executable()?;
    let mut cmd = Command::new(&executable);
    cmd.arg(format!("--user-data-dir={}", user_data_dir.display()))
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .arg("--disable-default-apps")
        .arg("--new-window");

    if let Some(port) = options.debug_port {
        cmd.arg(format!("--remote-debugging-port={port}"));
        cmd.arg("--remote-debugging-address=127.0.0.1");
    }

    if options.headless {
        cmd.arg("--headless=new");
        cmd.arg("--disable-gpu");
    }

    if let Some(url) = &options.start_url {
        cmd.arg(url);
    } else {
        cmd.arg("about:blank");
    }

    Ok(cmd)
}

/// Poll until Chrome answers CDP on loopback (or the child exits / timeout).
pub async fn wait_for_cdp_ready(
    port: u16,
    timeout: Duration,
    child: &mut tokio::process::Child,
    user_data_dir: &Path,
) -> Result<(), String> {
    let url = format!("http://127.0.0.1:{port}/json/version");
    let deadline = tokio::time::Instant::now() + timeout;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .map_err(|e| format!("Failed to build CDP readiness client: {e}"))?;

    loop {
        match child.try_wait() {
            Ok(Some(_status)) => return Err(attach_failed_before_cdp_error(user_data_dir)),
            Ok(None) => {}
            Err(error) => {
                warn!("Failed to poll Chrome child while waiting for CDP: {error}");
            }
        }

        match client.get(&url).send().await {
            Ok(response) if response.status().is_success() => return Ok(()),
            Ok(response) => {
                warn!(
                    "CDP endpoint returned status {} while waiting on port {port}",
                    response.status()
                );
            }
            Err(error) => {
                debug_log_cdp_wait(&error);
            }
        }
        if tokio::time::Instant::now() >= deadline {
            if chrome_profile_appears_in_use(user_data_dir) {
                return Err(profile_in_use_error());
            }
            return Err(format!(
                "Timed out waiting for Chrome remote debugging on 127.0.0.1:{port}. {PROFILE_IN_USE_HINT}"
            ));
        }
        tokio::time::sleep(Duration::from_millis(150)).await;
    }
}

fn debug_log_cdp_wait(error: &reqwest::Error) {
    log::debug!("Waiting for CDP endpoint: {error}");
}

/// Open the imported Chromium profile in real Chrome for **manual** sign-in (no CDP).
///
/// Registry name only — never accepts filesystem paths from the UI.
pub fn open_imported_profile_for_signin(name: &str) -> Result<(), String> {
    let registry = load_registry()?;
    let profile = registry.profiles.get(name).ok_or_else(|| {
        format!("Imported browser profile '{name}' was not found. Import or create one from Settings first.")
    })?;

    if profile.import_kind == ImportKind::FirefoxCookies {
        return Err(
            "Firefox imports inject cookies into Chromium automation and cannot open a Firefox window for sign-in. Prefer a Chrome/Edge/Brave import, or sign in inside an agent session after approval."
                .to_string(),
        );
    }

    let user_data_dir = ensure_under_profiles_storage(&profile.user_data_dir)?;
    if !user_data_dir.is_dir() {
        return Err(
            "Imported browser profile directory is missing. Re-import from Settings.".to_string(),
        );
    }

    if chrome_profile_appears_in_use(&user_data_dir) {
        return Err(profile_in_use_error());
    }

    // No remote-debugging-port: Google login must happen without CDP automation.
    let child = spawn_system_chrome_for_profile(&ChromeProfileSpawnOptions {
        user_data_dir,
        debug_port: None,
        headless: false,
        start_url: Some("https://accounts.google.com/".to_string()),
    })?;

    // Chrome wrappers on Linux often exit after forking the real browser. Reap the
    // launcher on a background thread so we do not leave zombies. Windows already
    // detaches when debug_port is None.
    #[cfg(unix)]
    {
        std::thread::spawn(move || {
            let mut child = child;
            let _ = child.wait();
        });
    }
    #[cfg(windows)]
    {
        // Detached process — drop without waiting.
        drop(child);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser_profiles::registry::profiles_storage_root;
    use std::ffi::OsStr;

    #[test]
    fn picks_nonzero_loopback_port() {
        let port = pick_loopback_debug_port().expect("port");
        assert_ne!(port, 0);
    }

    #[test]
    fn candidate_list_is_nonempty_on_macos_and_windows() {
        if cfg!(target_os = "macos") || cfg!(windows) {
            assert!(!system_chrome_candidate_paths().is_empty());
        }
    }

    #[test]
    fn build_command_rejects_paths_outside_profiles_storage() {
        let outside = std::env::temp_dir().join("libragent-chrome-attach-outside");
        let err = build_system_chrome_command(&ChromeProfileSpawnOptions {
            user_data_dir: outside,
            debug_port: Some(9222),
            headless: false,
            start_url: None,
        })
        .expect_err("must reject paths outside profiles storage");
        assert!(
            err.to_lowercase().contains("profile") || err.to_lowercase().contains("storage"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn build_command_includes_debug_and_user_data_when_executable_exists() {
        let Ok(executable) = resolve_system_chrome_executable() else {
            // CI images without Chrome/Edge skip this integration-style check.
            return;
        };
        let root = profiles_storage_root().expect("profiles root");
        let dir = root.join("unit-test-signin-profile");
        std::fs::create_dir_all(&dir).expect("mkdir");

        let cmd = build_system_chrome_command(&ChromeProfileSpawnOptions {
            user_data_dir: dir.clone(),
            debug_port: Some(9333),
            headless: true,
            start_url: Some("https://accounts.google.com/".to_string()),
        })
        .expect("build command");

        let program = cmd.get_program();
        assert_eq!(program, OsStr::new(&executable));

        let args: Vec<String> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert!(args.iter().any(|a| a.starts_with("--user-data-dir=")));
        assert!(args.iter().any(|a| a == "--remote-debugging-port=9333"));
        assert!(args
            .iter()
            .any(|a| a == "--remote-debugging-address=127.0.0.1"));
        assert!(args.iter().any(|a| a == "--headless=new"));
        assert!(args.iter().any(|a| a == "https://accounts.google.com/"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn parse_singleton_pid_from_chrome_lock_target() {
        assert_eq!(
            parse_chrome_singleton_pid(OsStr::new("myhost-12345")),
            Some(12345)
        );
        assert_eq!(parse_chrome_singleton_pid(OsStr::new("bad")), None);
    }

    #[test]
    fn profile_not_in_use_when_lock_missing() {
        let dir = std::env::temp_dir().join("libragent-chrome-attach-no-lock");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        assert!(!chrome_profile_appears_in_use(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn profile_in_use_when_singleton_points_at_self() {
        let dir = std::env::temp_dir().join("libragent-chrome-attach-self-lock");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        let lock = dir.join("SingletonLock");
        let target = format!("testhost-{}", std::process::id());
        std::os::unix::fs::symlink(&target, &lock).expect("symlink");
        assert!(chrome_profile_appears_in_use(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
