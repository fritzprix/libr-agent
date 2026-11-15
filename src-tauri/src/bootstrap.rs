//! Process bootstrap shared by every entry point (#2761).
//!
//! The GUI (`main.rs`), `liblet`, and `libragent-tui` all need the same
//! startup order before they can run: dotenv files, the runtime profile,
//! the data directories, and the backend descriptor namespace. Extracting
//! it keeps the entries one screen long and impossible to drift apart.

use crate::profile::{self, AppProfile};

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
    crate::server::backend_descriptor::init(&db_path);

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
