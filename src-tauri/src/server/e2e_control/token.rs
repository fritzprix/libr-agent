//! Capability token for the E2E control plane (debug builds only).
//!
//! The token is minted once per process and written to `<data_dir>/e2e_control.cap`
//! so the harness can read it back. Two conditions must hold before a token
//! exists at all:
//!
//! 1. The binary is a **debug** build (`debug_assertions`). Release builds never
//!    even compile this module (see `server/mod.rs`), so the endpoint is
//!    structurally absent from production binaries.
//! 2. The operator opted in with `LIBRAGENT_E2E_CONTROL=1` **and** pointed
//!    `LIBRAGENT_DATA_DIR` at an explicit (disposable) data directory. Without
//!    an explicit data dir the capability file could land in the user's real
//!    profile, so the control plane stays disabled instead.
//!
//! The token is written with create-exclusive semantics: a second process on
//! the same data dir fails to write and therefore stays disabled rather than
//! sharing or overwriting the first process's token.

use std::io::Write;
use std::sync::OnceLock;

pub const E2E_CONTROL_HEADER: &str = "x-libragent-e2e-control";
pub const E2E_CONTROL_CAPABILITY_FILE: &str = "e2e_control.cap";

static CAPABILITY: OnceLock<Option<String>> = OnceLock::new();

/// The explicit data-directory override, if the operator set one.
fn explicit_data_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("LIBRAGENT_DATA_DIR")
        .filter(|value| !value.is_empty())
        .map(std::path::PathBuf::from)
}

/// True when this binary is a debug build and the operator opted in via env
/// with an explicit (disposable) data directory.
pub fn e2e_control_enabled() -> bool {
    cfg!(debug_assertions)
        && std::env::var("LIBRAGENT_E2E_CONTROL").ok().as_deref() == Some("1")
        && explicit_data_dir().is_some()
}

/// Write the token to `<dir>/e2e_control.cap`, failing if the file exists.
///
/// Unix: created with mode 0600 via `OpenOptions::mode`. Windows: the file
/// inherits the (per-user) ACL of the data directory, which is disposable and
/// instance-owned under `LIBRAGENT_DATA_DIR`.
fn write_capability_file(dir: &std::path::Path, token: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(E2E_CONTROL_CAPABILITY_FILE);
    let mut options = std::fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&path)?;
    file.write_all(token.as_bytes())
}

/// Mint and store the capability token once per process (debug + opt-in only).
pub fn init_capability_token() {
    if !e2e_control_enabled() {
        let _ = CAPABILITY.set(None);
        return;
    }
    let dir = explicit_data_dir().expect("e2e_control_enabled checked this");
    let token = format!(
        "{}{}",
        uuid::Uuid::new_v4().to_string().replace('-', ""),
        uuid::Uuid::new_v4().to_string().replace('-', "")
    );
    match write_capability_file(&dir, &token) {
        Ok(()) => {
            let _ = CAPABILITY.set(Some(token));
            log::warn!(
                "E2E control HTTP enabled (loopback + capability header). Debug builds only."
            );
        }
        Err(error) => {
            // A stale or foreign cap file means another process owns this data
            // dir: stay disabled rather than sharing or replacing its token.
            // Name the recovery so a `pnpm tauri dev` restart is self-explanatory.
            log::error!(
                "E2E control disabled: cannot write capability file {}/{E2E_CONTROL_CAPABILITY_FILE}: {error}. \
                 If no other instance is running on this LIBRAGENT_DATA_DIR, delete the file and restart to re-enable.",
                dir.display()
            );
            let _ = CAPABILITY.set(None);
        }
    }
}

/// The process capability token, when the control plane is enabled.
pub fn capability_token() -> Option<&'static str> {
    CAPABILITY.get().and_then(|value| value.as_deref())
}

/// Compare two strings without early-exit on the first differing byte.
pub fn constant_time_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let mut diff = (a.len() ^ b.len()) as u8;
    for i in 0..a.len().max(b.len()) {
        let x = a.get(i).copied().unwrap_or(0);
        let y = b.get(i).copied().unwrap_or(0);
        diff |= x ^ y;
    }
    diff == 0
}

/// True when the request carries the exact capability header value.
pub fn header_matches(headers: &warp::http::HeaderMap, capability: &str) -> bool {
    headers
        .get(E2E_CONTROL_HEADER)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|provided| constant_time_eq(provided.trim(), capability))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_time_eq_accepts_equal_and_rejects_unequal() {
        assert!(constant_time_eq("abc123", "abc123"));
        assert!(!constant_time_eq("abc123", "abc124"));
        assert!(!constant_time_eq("abc123", "abc12"));
        assert!(!constant_time_eq("", "a"));
        assert!(constant_time_eq("", ""));
    }

    #[test]
    fn header_matches_requires_exact_capability() {
        let mut headers = warp::http::HeaderMap::new();
        headers.insert(E2E_CONTROL_HEADER, "tok123".parse().unwrap());
        assert!(header_matches(&headers, "tok123"));
        assert!(!header_matches(&headers, "tok124"));
        assert!(!header_matches(&warp::http::HeaderMap::new(), "tok123"));
    }
}
