//! Runtime profile: `prod` | `dev` | `demo`.
//!
//! - **prod** (release default): `…/com.fritzprix.libragent` + `libragent_v2.db`
//! - **dev** (debug default): same data dir + `libragent_v2.dev.db` (backward compatible)
//! - **demo**: isolated `…/com.fritzprix.libragent-demo` + `libragent_v2.demo.db`
//!
//! Override with `LIBRAGENT_PROFILE`, `--profile <name>`, or `--demo`.
//! Paths: `LIBRAGENT_DATA_DIR`, `LIBRAGENT_DB_PATH`.

use std::env;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppProfile {
    Prod,
    Dev,
    Demo,
}

impl AppProfile {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Prod => "prod",
            Self::Dev => "dev",
            Self::Demo => "demo",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "prod" | "production" => Some(Self::Prod),
            "dev" | "development" => Some(Self::Dev),
            "demo" => Some(Self::Demo),
            _ => None,
        }
    }
}

/// Resolve active profile from CLI / env / build type.
pub fn resolve_profile() -> AppProfile {
    let args: Vec<String> = env::args().collect();

    if args.iter().any(|a| a == "--demo") {
        return AppProfile::Demo;
    }

    if let Some(idx) = args.iter().position(|a| a == "--profile") {
        if let Some(value) = args.get(idx + 1) {
            if let Some(profile) = AppProfile::parse(value) {
                return profile;
            }
            eprintln!("⚠️  Unknown --profile '{value}' (expected prod|dev|demo); ignoring");
        }
    }

    if let Ok(raw) = env::var("LIBRAGENT_PROFILE") {
        if let Some(profile) = AppProfile::parse(&raw) {
            return profile;
        }
        eprintln!("⚠️  Unknown LIBRAGENT_PROFILE='{raw}' (expected prod|dev|demo); ignoring");
    }

    if cfg!(debug_assertions) {
        AppProfile::Dev
    } else {
        AppProfile::Prod
    }
}

/// Base data directory for sessions, skills, and related state.
pub fn data_dir(profile: AppProfile) -> PathBuf {
    if let Ok(override_dir) = env::var("LIBRAGENT_DATA_DIR") {
        return PathBuf::from(override_dir);
    }

    let home_data = dirs::data_dir().expect("Failed to get system data directory");
    match profile {
        AppProfile::Prod | AppProfile::Dev => home_data.join("com.fritzprix.libragent"),
        AppProfile::Demo => home_data.join("com.fritzprix.libragent-demo"),
    }
}

fn default_db_file_name(profile: AppProfile) -> &'static str {
    match profile {
        AppProfile::Prod => "libragent_v2.db",
        AppProfile::Dev => "libragent_v2.dev.db",
        AppProfile::Demo => "libragent_v2.demo.db",
    }
}

/// SQLite database file path for this profile.
pub fn db_path(profile: AppProfile) -> PathBuf {
    if let Ok(override_path) = env::var("LIBRAGENT_DB_PATH") {
        return PathBuf::from(override_path);
    }
    data_dir(profile).join(default_db_file_name(profile))
}

fn env_flag_truthy(name: &str) -> bool {
    env::var(name)
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

/// Whether demo profile should wipe its data dir before boot (clean slate).
///
/// Default for demo: reset unless `LIBRAGENT_DEMO_PERSIST=1`.
/// Explicit `LIBRAGENT_DEMO_RESET=1` always resets.
pub fn should_reset_demo(profile: AppProfile) -> bool {
    if profile != AppProfile::Demo {
        return false;
    }
    if env_flag_truthy("LIBRAGENT_DEMO_RESET") {
        return true;
    }
    if env_flag_truthy("LIBRAGENT_DEMO_PERSIST") {
        return false;
    }
    // Fresh demo by default
    true
}

/// Wipe demo data directory when requested. No-op for other profiles.
pub fn maybe_reset_demo_data(profile: AppProfile) -> Result<(), String> {
    if !should_reset_demo(profile) {
        return Ok(());
    }
    let dir = data_dir(profile);
    if dir.exists() {
        fs::remove_dir_all(&dir)
            .map_err(|e| format!("Failed to reset demo data dir {}: {e}", dir.display()))?;
        println!("🧹 Demo profile: reset data dir {}", dir.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_profile_names() {
        assert_eq!(AppProfile::parse("demo"), Some(AppProfile::Demo));
        assert_eq!(AppProfile::parse("DEV"), Some(AppProfile::Dev));
        assert_eq!(AppProfile::parse("production"), Some(AppProfile::Prod));
        assert_eq!(AppProfile::parse("nope"), None);
    }
}
