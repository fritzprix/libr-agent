use url::Url;

fn env_option(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// Whether `NO_PROXY` (or `no_proxy`) says to skip the proxy for this URL.
pub fn no_proxy_bypasses(url: &str) -> bool {
    let Some(list) = env_option("NO_PROXY").or_else(|| env_option("no_proxy")) else {
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
        // Leading-dot suffix form: `.corp` matches `host.corp`
        if let Some(suffix) = entry.strip_prefix('.') {
            if let Ok(parsed) = Url::parse(url) {
                if let Some(host) = parsed.host_str() {
                    if host.ends_with(entry) || host.eq_ignore_ascii_case(suffix) {
                        return true;
                    }
                }
            }
            continue;
        }
        let pattern = entry.trim_start_matches('*');
        if url.contains(pattern) {
            return true;
        }
    }
    false
}

fn normalize_proxy_url(raw: &str) -> String {
    let raw = raw.trim();
    if raw.contains("://") {
        raw.to_string()
    } else {
        format!("http://{raw}")
    }
}

/// Env-based proxy for `url`, if any (`HTTP_PROXY` / `HTTPS_PROXY` / `ALL_PROXY`).
pub fn env_http_proxy_for_url(url: &Url) -> Option<String> {
    if no_proxy_bypasses(url.as_str()) {
        return None;
    }
    let proxy = if url.scheme() == "https" {
        env_option("HTTPS_PROXY")
            .or_else(|| env_option("https_proxy"))
            .or_else(|| env_option("ALL_PROXY"))
            .or_else(|| env_option("all_proxy"))
    } else {
        env_option("HTTP_PROXY")
            .or_else(|| env_option("http_proxy"))
            .or_else(|| env_option("HTTPS_PROXY"))
            .or_else(|| env_option("https_proxy"))
            .or_else(|| env_option("ALL_PROXY"))
            .or_else(|| env_option("all_proxy"))
    };
    proxy.map(|p| normalize_proxy_url(&p))
}