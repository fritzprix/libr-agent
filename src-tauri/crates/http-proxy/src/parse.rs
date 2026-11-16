//! Parse WinHTTP / PAC-style proxy result strings.

use crate::env::host_matches_no_proxy_entry;
use url::Url;

/// Proxy chain resolved for a specific target URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedProxy {
    Direct,
    Proxies(Vec<String>),
}

impl ResolvedProxy {
    pub fn first_reqwest_proxy_url(&self) -> Option<String> {
        match self {
            ResolvedProxy::Direct => None,
            ResolvedProxy::Proxies(list) => list.first().cloned(),
        }
    }
}

fn normalize_proxy_host_port(host_port: &str, scheme_hint: &str) -> Option<String> {
    let host_port = host_port.trim();
    if host_port.is_empty() {
        return None;
    }
    if host_port.contains("://") {
        return Some(host_port.to_string());
    }
    let prefix = if scheme_hint.eq_ignore_ascii_case("socks")
        || scheme_hint.eq_ignore_ascii_case("socks5")
    {
        "socks5"
    } else {
        "http"
    };
    Some(format!("{prefix}://{host_port}"))
}

/// Parse a PAC-style list (`PROXY a:8080; DIRECT`) into reqwest proxy URLs.
pub fn parse_proxy_list(raw: &str) -> Vec<String> {
    let mut out = Vec::new();
    for part in raw.split([';', ';']) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let upper = part.to_uppercase();
        if upper == "DIRECT" {
            continue;
        }
        if upper.starts_with("PROXY ") {
            if let Some(url) = normalize_proxy_host_port(part[6..].trim(), "http") {
                out.push(url);
            }
            continue;
        }
        if upper.starts_with("SOCKS5 ") {
            if let Some(url) = normalize_proxy_host_port(part[7..].trim(), "socks5") {
                out.push(url);
            }
            continue;
        }
        if upper.starts_with("SOCKS ") {
            if let Some(url) = normalize_proxy_host_port(part[6..].trim(), "socks5") {
                out.push(url);
            }
            continue;
        }
        // Bare host:port (PAC without keyword). Ignore IE scheme=key forms here.
        if !part.contains('=') {
            if let Some(url) = normalize_proxy_host_port(part, "http") {
                out.push(url);
            }
        }
    }
    out
}

/// Parse an IE/WinHTTP static proxy string for a specific request scheme.
///
/// Supports:
/// - single proxy: `proxy.corp:8080`
/// - per-scheme: `http=a:80;https=b:443;socks=s:1080`
pub fn parse_static_proxy_for_scheme(raw: &str, request_scheme: &str) -> ResolvedProxy {
    let raw = raw.trim();
    if raw.is_empty() {
        return ResolvedProxy::Direct;
    }

    let has_scheme_keys = raw.split(';').any(|p| {
        let p = p.trim().to_ascii_lowercase();
        p.starts_with("http=")
            || p.starts_with("https=")
            || p.starts_with("ftp=")
            || p.starts_with("socks=")
    });

    if !has_scheme_keys {
        let list = parse_proxy_list(raw);
        return if list.is_empty() {
            ResolvedProxy::Direct
        } else {
            ResolvedProxy::Proxies(list)
        };
    }

    let want = request_scheme.to_ascii_lowercase();
    let mut chosen: Option<String> = None;
    let mut socks: Option<String> = None;

    for part in raw.split(';') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        match key.as_str() {
            "http" if want == "http" => {
                chosen = normalize_proxy_host_port(value, "http");
            }
            "https" if want == "https" => {
                chosen = normalize_proxy_host_port(value, "http");
            }
            "socks" => {
                socks = normalize_proxy_host_port(value, "socks5");
            }
            _ => {}
        }
    }

    // Do not apply the http= entry to https requests.
    if let Some(url) = chosen.or(socks) {
        ResolvedProxy::Proxies(vec![url])
    } else {
        ResolvedProxy::Direct
    }
}

/// Whether a WinHTTP / IE bypass list says to skip the proxy for `url`.
///
/// Honors `<local>` (host without a dot) and semicolon/comma-separated host patterns.
pub fn proxy_bypass_matches(url: &str, bypass_list: &str) -> bool {
    let Ok(parsed) = Url::parse(url) else {
        return false;
    };
    let Some(host) = parsed.host_str() else {
        return false;
    };
    let host_l = host.to_ascii_lowercase();
    let port = parsed.port();

    for entry in bypass_list.split([';', ';']) {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }
        if entry.eq_ignore_ascii_case("<local>") {
            if !host_l.contains('.') {
                return true;
            }
            continue;
        }
        if host_matches_no_proxy_entry(host, port, entry) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scheme_specific_static_proxy_picks_https_entry() {
        let raw = "http=a:80;https=b:443";
        assert_eq!(
            parse_static_proxy_for_scheme(raw, "https"),
            ResolvedProxy::Proxies(vec!["http://b:443".into()])
        );
        assert_eq!(
            parse_static_proxy_for_scheme(raw, "http"),
            ResolvedProxy::Proxies(vec!["http://a:80".into()])
        );
    }

    #[test]
    fn bypass_list_honors_local_and_host() {
        assert!(proxy_bypass_matches(
            "http://intranet/page",
            "<local>;*.corp"
        ));
        assert!(proxy_bypass_matches("https://app.corp/x", "<local>;.corp"));
        assert!(!proxy_bypass_matches(
            "https://api.openai.com/v1",
            "<local>;.corp"
        ));
    }
}
