//! Firefox cookie export for Chromium CDP injection.
//!
//! LibrAgent automation launches Chromium, so Firefox User Data cannot be used as
//! `--user-data-dir`. Instead we copy cookies from `cookies.sqlite` into a JSON
//! file that the sidecar injects via CDP before navigation.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};

pub const INJECT_COOKIES_FILE: &str = "libragent_inject_cookies.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InjectableCookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    pub secure: bool,
    pub http_only: bool,
    /// `None` | `Lax` | `Strict`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub same_site: Option<String>,
    /// Unix expiry seconds; omit for session cookies.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires: Option<f64>,
    /// Absolute URL used by CDP when the page is still on about:blank.
    pub url: String,
}

/// Read Firefox `cookies.sqlite` and write injectable JSON under `dest_user_data`.
pub fn export_firefox_cookies_to_profile(
    firefox_profile_dir: &Path,
    dest_user_data: &Path,
) -> Result<(usize, Vec<String>), String> {
    let cookies = read_firefox_cookies(firefox_profile_dir)?;
    if cookies.is_empty() {
        return Err(
            "Firefox profile has no cookies to import. Sign in to sites in Firefox, then retry."
                .to_string(),
        );
    }

    std::fs::create_dir_all(dest_user_data)
        .map_err(|e| format!("Failed to create Firefox import destination: {e}"))?;
    // Minimal Chromium User Data so --user-data-dir is valid; cookies arrive via CDP.
    let default_dir = dest_user_data.join("Default");
    std::fs::create_dir_all(&default_dir)
        .map_err(|e| format!("Failed to create Chromium Default stub: {e}"))?;
    if !default_dir.join("Preferences").is_file() {
        std::fs::write(default_dir.join("Preferences"), b"{}")
            .map_err(|e| format!("Failed to write Preferences stub: {e}"))?;
    }

    let path = dest_user_data.join(INJECT_COOKIES_FILE);
    let raw = serde_json::to_string_pretty(&cookies)
        .map_err(|e| format!("Failed to serialize Firefox cookies: {e}"))?;
    std::fs::write(&path, raw)
        .map_err(|e| format!("Failed to write injectable cookie file: {e}"))?;

    let warnings = vec![
        "Firefox import copies cookies only (not passwords, localStorage, or extensions). Automation still runs Chromium.".to_string(),
    ];
    Ok((cookies.len(), warnings))
}

pub fn read_inject_cookies_file(user_data_dir: &Path) -> Result<Option<Vec<InjectableCookie>>, String> {
    let path = user_data_dir.join(INJECT_COOKIES_FILE);
    if !path.is_file() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read injectable cookies: {e}"))?;
    let cookies: Vec<InjectableCookie> = serde_json::from_str(&raw)
        .map_err(|e| format!("Failed to parse injectable cookies: {e}"))?;
    Ok(Some(cookies))
}

fn read_firefox_cookies(profile_dir: &Path) -> Result<Vec<InjectableCookie>, String> {
    let source = profile_dir.join("cookies.sqlite");
    if !source.is_file() {
        return Err(format!(
            "Firefox cookies database not found at {}",
            source.display()
        ));
    }

    let tmp = copy_sqlite_for_read(&source)?;
    let conn = Connection::open_with_flags(&tmp, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("Failed to open Firefox cookies database (is Firefox running?): {e}")
    })?;

    let result = query_cookies(&conn);
    drop(conn);
    let _ = std::fs::remove_file(&tmp);
    let _ = std::fs::remove_file(sqlite_companion(&tmp, "-wal"));
    let _ = std::fs::remove_file(sqlite_companion(&tmp, "-shm"));
    result
}

fn copy_sqlite_for_read(source: &Path) -> Result<PathBuf, String> {
    let tmp = std::env::temp_dir().join(format!(
        "libragent_ff_cookies_{}_{}.sqlite",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    ));
    std::fs::copy(source, &tmp)
        .map_err(|e| format!("Failed to copy Firefox cookies database: {e}"))?;
    // Best-effort WAL companions so an open Firefox still yields a consistent snapshot.
    let wal_src = sqlite_companion(source, "-wal");
    let shm_src = sqlite_companion(source, "-shm");
    if wal_src.is_file() {
        let _ = std::fs::copy(&wal_src, sqlite_companion(&tmp, "-wal"));
    }
    if shm_src.is_file() {
        let _ = std::fs::copy(&shm_src, sqlite_companion(&tmp, "-shm"));
    }
    Ok(tmp)
}

fn sqlite_companion(path: &Path, suffix: &str) -> PathBuf {
    let mut os = path.as_os_str().to_owned();
    os.push(suffix);
    PathBuf::from(os)
}

fn query_cookies(conn: &Connection) -> Result<Vec<InjectableCookie>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT name, value, host, path, expiry, isSecure, isHttpOnly, sameSite
             FROM moz_cookies
             WHERE name IS NOT NULL AND value IS NOT NULL AND host IS NOT NULL",
        )
        .map_err(|e| format!("Failed to query Firefox cookies: {e}"))?;

    let rows = stmt
        .query_map([], |row| {
            let name: String = row.get(0)?;
            let value: String = row.get(1)?;
            let host: String = row.get(2)?;
            let path: String = row.get(3)?;
            let expiry: i64 = row.get(4)?;
            let is_secure: i64 = row.get(5)?;
            let is_http_only: i64 = row.get(6)?;
            let same_site: i64 = row.get(7)?;
            Ok((
                name,
                value,
                host,
                path,
                expiry,
                is_secure != 0,
                is_http_only != 0,
                same_site,
            ))
        })
        .map_err(|e| format!("Failed to read Firefox cookie rows: {e}"))?;

    let mut cookies = Vec::new();
    for row in rows {
        let (name, value, host, path, expiry, secure, http_only, same_site) =
            row.map_err(|e| format!("Failed to decode Firefox cookie row: {e}"))?;
        if name.is_empty() || host.is_empty() {
            continue;
        }
        let domain = host;
        let url = cookie_url_for_domain(&domain, secure);
        let same_site = match same_site {
            // Firefox: 1=Lax, 2=Strict. 0 is historically "unset" on older DBs and
            // also SAMESITE_NONE on modern builds — omit the attribute so Chromium
            // applies its default instead of forcing SameSite=None+Secure.
            1 => Some("Lax".to_string()),
            2 => Some("Strict".to_string()),
            _ => None,
        };
        // Firefox expiry is unix seconds; 0 often means session.
        let expires = if expiry > 0 {
            Some(expiry as f64)
        } else {
            None
        };
        cookies.push(InjectableCookie {
            name,
            value,
            domain,
            path: if path.is_empty() {
                "/".to_string()
            } else {
                path
            },
            secure,
            http_only,
            same_site,
            expires,
            url,
        });
    }
    Ok(cookies)
}

fn cookie_url_for_domain(domain: &str, secure: bool) -> String {
    let host = domain.trim().trim_start_matches('.');
    let scheme = if secure { "https" } else { "http" };
    format!("{scheme}://{host}/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_https_url_for_secure_cookie_domains() {
        assert_eq!(
            cookie_url_for_domain(".example.com", true),
            "https://example.com/"
        );
        assert_eq!(
            cookie_url_for_domain("www.example.com", false),
            "http://www.example.com/"
        );
    }
}
