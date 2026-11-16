use url::Url;

fn env_option(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// Whether `NO_PROXY` / `no_proxy` says to skip the proxy for this URL.
///
/// Matching follows reqwest/`curl` host rules: exact host, optional port,
/// `.suffix` / `*.suffix`, and `*` — never a substring of the full URL.
pub fn no_proxy_bypasses(url: &str) -> bool {
    let Some(list) = env_option("NO_PROXY").or_else(|| env_option("no_proxy")) else {
        return false;
    };
    let Ok(parsed) = Url::parse(url) else {
        return false;
    };
    let Some(host) = parsed.host_str() else {
        return false;
    };
    for entry in list.split([',', ';']) {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }
        if entry == "*" {
            return true;
        }
        if host_matches_no_proxy_entry(host, parsed.port(), entry) {
            return true;
        }
    }
    false
}

pub(crate) fn host_matches_no_proxy_entry(host: &str, port: Option<u16>, entry: &str) -> bool {
    let (entry_host, entry_port) = match entry.rsplit_once(':') {
        Some((h, p)) if p.chars().all(|c| c.is_ascii_digit()) => {
            (h, p.parse::<u16>().ok())
        }
        _ => (entry, None),
    };

    if let Some(ep) = entry_port {
        if port != Some(ep) {
            return false;
        }
    }

    let host_l = host.to_ascii_lowercase();
    let entry_l = entry_host.to_ascii_lowercase();

    if entry_l.starts_with('.') {
        let suffix = &entry_l[1..];
        return host_l == *suffix || host_l.ends_with(&entry_l);
    }
    if let Some(suffix) = entry_l.strip_prefix("*.") {
        return host_l == *suffix || host_l.ends_with(&format!(".{suffix}"));
    }
    host_l == entry_l
}

fn normalize_proxy_url(raw: &str) -> String {
    let raw = raw.trim();
    if raw.contains("://") {
        raw.to_string()
    } else {
        format!("http://{raw}")
    }
}

/// Env-based proxy for `url`, matching reqwest's scheme rules (no cross-scheme
/// fallback from `HTTPS_PROXY` onto plain `http://` URLs).
pub fn env_http_proxy_for_url(url: &Url) -> Option<String> {
    if no_proxy_bypasses(url.as_str()) {
        return None;
    }
    let proxy = if url.scheme() == "https" {
        // Match reqwest: HTTPS_PROXY / ALL_PROXY only (no HTTP_PROXY fallback).
        env_option("HTTPS_PROXY")
            .or_else(|| env_option("https_proxy"))
            .or_else(|| env_option("ALL_PROXY"))
            .or_else(|| env_option("all_proxy"))
    } else if url.scheme() == "http" {
        env_option("HTTP_PROXY")
            .or_else(|| env_option("http_proxy"))
            .or_else(|| env_option("ALL_PROXY"))
            .or_else(|| env_option("all_proxy"))
    } else {
        env_option("ALL_PROXY").or_else(|| env_option("all_proxy"))
    };
    proxy.map(|p| normalize_proxy_url(&p))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn no_proxy_matches_host_not_path_or_query() {
        let _g = ENV_LOCK.lock().unwrap();
        std::env::set_var("NO_PROXY", "internal,10.0.0.1,local");
        assert!(
            !no_proxy_bypasses("https://api.openai.com/v1/internal/x"),
            "path must not match"
        );
        assert!(
            !no_proxy_bypasses("https://example.com/?ip=10.0.0.1"),
            "query must not match"
        );
        assert!(
            !no_proxy_bypasses("https://api.local-llm.com"),
            "substring of host label must not match"
        );
        assert!(no_proxy_bypasses("https://internal/v1"));
        assert!(no_proxy_bypasses("https://10.0.0.1/foo"));
        assert!(no_proxy_bypasses("https://local/"));
        std::env::remove_var("NO_PROXY");
    }

    #[test]
    fn http_urls_do_not_fall_back_to_https_proxy() {
        let _g = ENV_LOCK.lock().unwrap();
        std::env::remove_var("HTTP_PROXY");
        std::env::remove_var("http_proxy");
        std::env::remove_var("ALL_PROXY");
        std::env::remove_var("all_proxy");
        std::env::set_var("HTTPS_PROXY", "http://https-only:443");
        let url = Url::parse("http://example.com/").unwrap();
        assert!(env_http_proxy_for_url(&url).is_none());
        std::env::remove_var("HTTPS_PROXY");
    }

    #[test]
    fn https_urls_do_not_fall_back_to_http_proxy() {
        let _g = ENV_LOCK.lock().unwrap();
        std::env::remove_var("HTTPS_PROXY");
        std::env::remove_var("https_proxy");
        std::env::remove_var("ALL_PROXY");
        std::env::remove_var("all_proxy");
        std::env::set_var("HTTP_PROXY", "http://http-only:8080");
        let url = Url::parse("https://example.com/").unwrap();
        assert!(env_http_proxy_for_url(&url).is_none());
        std::env::remove_var("HTTP_PROXY");
    }
}
