//! Legacy-only reader for `libragent_inject_cookies.json` left by older Firefox imports.
//!
//! New imports are Chromium-only (Chrome / Edge / Brave). Registry defaults reject
//! `FirefoxCookies`, so agents should not rely on this path — prefer deleting the
//! legacy entry and importing a Chromium profile. The reader remains so an old
//! on-disk inject file does not panic if still present.

use std::path::Path;

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

pub fn read_inject_cookies_file(
    user_data_dir: &Path,
) -> Result<Option<Vec<InjectableCookie>>, String> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_none_when_inject_file_missing() {
        let dir = std::env::temp_dir().join("libragent-ff-inject-missing");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        assert!(read_inject_cookies_file(&dir).unwrap().is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn parses_inject_file() {
        let dir = std::env::temp_dir().join("libragent-ff-inject-present");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(INJECT_COOKIES_FILE);
        std::fs::write(
            &path,
            r#"[{"name":"a","value":"b","domain":".example.com","path":"/","secure":true,"httpOnly":false,"url":"https://example.com/"}]"#,
        )
        .unwrap();
        let cookies = read_inject_cookies_file(&dir).unwrap().unwrap();
        assert_eq!(cookies.len(), 1);
        assert_eq!(cookies[0].name, "a");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
