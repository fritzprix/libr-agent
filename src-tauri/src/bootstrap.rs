//! Process bootstrap shared by every entry point (#2761).
//!
//! The GUI (`main.rs`), `liblet`, and `libragent-tui` all need the same
//! startup order before they can run: dotenv files, the runtime profile,
//! the data directories, and the backend descriptor namespace. Extracting
//! it keeps the entries one screen long and impossible to drift apart.

use std::path::{Path, PathBuf};

use crate::profile::{self, AppProfile};

/// Per-process WebView user-data dir for a backend that shares the profile
/// with another live backend.
///
/// `None` keeps the **default** WebView profile, which is what the first
/// backend on a profile must do: `webview/<pid>` would be new on every launch,
/// so localStorage, IndexedDB and cookies would start empty on every start
/// (#2858 G1.1).
fn webview_user_data_dir_for(
    data_dir: &Path,
    has_other_backend: bool,
    pid: u32,
) -> Option<PathBuf> {
    has_other_backend.then(|| data_dir.join("webview").join(pid.to_string()))
}

/// Remove `webview/<pid>` dirs whose process is gone. A directory can only be
/// removed once its owner is dead, so the sweep runs at start (#2858).
fn sweep_dead_webview_dirs_with(
    webview_root: &Path,
    own_pid: u32,
    pid_alive: impl Fn(u32) -> bool,
) -> usize {
    let Ok(entries) = std::fs::read_dir(webview_root) else {
        return 0;
    };
    let mut removed = 0;
    for entry in entries.flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<u32>().ok())
        else {
            continue;
        };
        if pid == own_pid || pid_alive(pid) {
            continue;
        }
        if std::fs::remove_dir_all(entry.path()).is_ok() {
            removed += 1;
        }
    }
    removed
}

fn pid_is_alive(pid: u32) -> bool {
    crate::utils::process::process_is_alive(pid).ok() == Some(true)
}

/// Isolate this backend's WebView user-data only when another live backend on
/// the same profile already owns the default one (#2858).
///
/// On Linux and macOS there is no per-process WebView data override: setting
/// `XDG_CACHE_HOME` process-wide would also redirect every child process's
/// cache (npx, uv, pip, the browser sidecar) into a throwaway directory, so
/// concurrent WebViews on one profile stay a documented limitation and the
/// second backend says so out loud instead of corrupting silently.
fn isolate_webview_user_data(data_dir: &Path) {
    let webview_root = data_dir.join("webview");
    sweep_dead_webview_dirs_with(&webview_root, std::process::id(), pid_is_alive);

    let target_dir = webview_user_data_dir_for(
        data_dir,
        crate::server::backend_descriptor::profile_has_other_backend(),
        std::process::id(),
    );

    let Some(webview_dir) = target_dir else {
        // First backend on the profile: keep the default WebView profile.
        return;
    };

    #[cfg(target_os = "windows")]
    {
        if let Err(error) = std::fs::create_dir_all(&webview_dir) {
            eprintln!(
                "⚠️  Warning: Failed to create WebView user-data dir {}: {error}",
                webview_dir.display()
            );
            return;
        }
        // Leave an explicit override alone (tests and smoke runs set their own).
        if std::env::var_os("WEBVIEW2_USER_DATA_FOLDER").is_none() {
            std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", &webview_dir);
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = &webview_dir;
        eprintln!(
            "⚠️  Another backend already runs on this profile. WebView cache sharing on this OS is a documented limitation (#2858); the window may need a reload."
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_backend_keeps_the_default_webview_profile() {
        let data_dir = Path::new("/data");
        assert_eq!(webview_user_data_dir_for(data_dir, false, 42), None);
    }

    #[test]
    fn second_backend_gets_a_private_webview_dir() {
        let data_dir = Path::new("/data");
        assert_eq!(
            webview_user_data_dir_for(data_dir, true, 42),
            Some(data_dir.join("webview").join("42"))
        );
    }

    #[test]
    fn sweep_removes_only_dead_process_webview_dirs() {
        let root = std::env::temp_dir().join(format!("webview-sweep-{}", std::process::id()));
        let own = root.join("1000");
        let dead = root.join("2000");
        let live = root.join("3000");
        for dir in [&own, &dead, &live] {
            std::fs::create_dir_all(dir).expect("create webview dir");
        }
        std::fs::write(dead.join("Cache"), b"x").expect("write cache file");
        std::fs::write(root.join("not-a-pid"), b"x").expect("write stray file");

        let removed = sweep_dead_webview_dirs_with(&root, 1000, |pid| pid == 3000);

        assert_eq!(removed, 1);
        assert!(own.exists(), "own dir must survive");
        assert!(!dead.exists(), "dead process dir must be removed");
        assert!(live.exists(), "live process dir must survive");
        assert!(
            root.join("not-a-pid").exists(),
            "non-pid entry must survive"
        );

        std::fs::remove_dir_all(&root).ok();
    }
}

/// True when the arguments or environment ask for the `demo` profile.
fn wants_demo_profile() -> bool {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--demo") {
        return true;
    }
    if let Some(idx) = args.iter().position(|a| a == "--profile") {
        if args
            .get(idx + 1)
            .is_some_and(|v| v.eq_ignore_ascii_case("demo"))
        {
            return true;
        }
    }
    std::env::var("LIBRAGENT_PROFILE")
        .map(|v| v.eq_ignore_ascii_case("demo"))
        .unwrap_or(false)
}

fn try_load_dotenv(path: &std::path::Path) -> bool {
    match dotenvy::from_path(path) {
        Ok(()) => {
            println!("✅ Loaded env file from: {}", path.display());
            true
        }
        Err(dotenvy::Error::Io(err)) if err.kind() == std::io::ErrorKind::NotFound => false,
        Err(e) => {
            eprintln!("⚠️  Warning: Failed to load {}: {e}", path.display());
            false
        }
    }
}

/// Load dotenv files. Demo prefers `.env.demo`; debug prefers `.env.dev`.
pub fn load_dotenv_files() {
    if wants_demo_profile() {
        let candidates = [
            std::path::PathBuf::from(".env.demo"),
            std::path::PathBuf::from("../.env.demo"),
        ];
        for path in &candidates {
            if try_load_dotenv(path) {
                return;
            }
        }
        println!("ℹ️  No .env.demo found (using system environment variables)");
        let _ = dotenvy::dotenv();
        return;
    }

    #[cfg(debug_assertions)]
    {
        match dotenvy::from_filename(".env.dev") {
            Ok(path) => println!("✅ Loaded .env.dev file from: {}", path.display()),
            Err(_) => match dotenvy::dotenv() {
                Ok(path) => println!("✅ Loaded .env file from: {}", path.display()),
                Err(dotenvy::Error::Io(err)) if err.kind() == std::io::ErrorKind::NotFound => {
                    println!(
                        "ℹ️  No .env or .env.dev file found (using system environment variables)"
                    );
                }
                Err(e) => {
                    eprintln!("⚠️  Warning: Failed to load .env file: {e}");
                }
            },
        }
    }

    #[cfg(not(debug_assertions))]
    {
        let loaded = match dotenvy::dotenv() {
            Ok(path) => {
                println!("✅ Loaded .env file from: {}", path.display());
                true
            }
            Err(dotenvy::Error::Io(err)) if err.kind() == std::io::ErrorKind::NotFound => {
                if let Ok(exe_path) = std::env::current_exe() {
                    if let Some(exe_dir) = exe_path.parent() {
                        let env_path = exe_dir.join(".env");
                        match dotenvy::from_path(&env_path) {
                            Ok(_) => {
                                println!("✅ Loaded .env file from: {}", env_path.display());
                                true
                            }
                            Err(_) => false,
                        }
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            Err(e) => {
                eprintln!("⚠️  Warning: Failed to load .env file: {e}");
                false
            }
        };

        if !loaded {
            println!("ℹ️  No .env file found (using system environment variables and defaults)");
        }
    }
}

/// Resolve the runtime profile, create its data directories, initialize the
/// backend descriptor namespace, and return the SQLite URL to open.
///
/// Prints the profile banner lines shared by every entry point.
pub fn prepare_profile_runtime() -> String {
    let profile = profile::resolve_profile();
    println!("📦 App profile: {}", profile.as_str());

    if let Err(err) = profile::maybe_reset_demo_data(profile) {
        eprintln!("❌ {err}");
        std::process::exit(1);
    }

    let db_path = profile::db_path(profile);
    let data_dir = profile::data_dir(profile);

    if let Some(parent_dir) = db_path.parent() {
        std::fs::create_dir_all(parent_dir).expect("Failed to create database directory");
    } else {
        std::fs::create_dir_all(&data_dir).expect("Failed to create data directory");
    }

    // Not a lock: this only tells the server where to publish its own record
    // and where to look for the records of other running backends (#2700).
    // A second app on the same profile starts normally and can find the
    // first one through these records.
    //
    // Runs before the WebView setup because that decision needs to know
    // whether another backend is already up on this profile (#2858 G1.1).
    crate::server::backend_descriptor::init(&db_path);

    // The first backend keeps the default WebView user-data dir; a backend
    // that finds another live one takes a private dir (#2858 G1.1).
    isolate_webview_user_data(&data_dir);

    let db_url = crate::utils::sqlite::format_sqlite_url(db_path.to_string_lossy().as_ref());

    match profile {
        AppProfile::Demo => {
            println!(
                "ℹ️  Demo profile: data={} db={}",
                data_dir.display(),
                db_path.display()
            );
        }
        AppProfile::Dev => {
            if std::env::var("LIBRAGENT_DB_PATH").is_err() {
                println!(
                    "ℹ️  Dev profile using isolated DB (set LIBRAGENT_DB_PATH to override): {}",
                    db_path.display()
                );
            }
        }
        AppProfile::Prod => {}
    }

    println!("🚀 Starting LibrAgent with SQLite database: {db_url}");
    db_url
}
