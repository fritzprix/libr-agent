//! Parse WinHTTP / PAC-style proxy result strings.

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

/// Parse WinHTTP / PAC-style proxy list strings into reqwest-ready proxy URLs.
pub fn parse_proxy_list(raw: &str) -> Vec<String> {
    let mut out = Vec::new();
    for part in raw.split(';') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let upper = part.to_uppercase();
        if upper == "DIRECT" {
            continue;
        }
        let host_port = if upper.starts_with("PROXY ") {
            part[6..].trim()
        } else if upper.starts_with("SOCKS ") {
            let hp = part[6..].trim();
            if !hp.is_empty() {
                out.push(format!("socks5://{hp}"));
            }
            continue;
        } else if upper.starts_with("HTTP=") {
            part[5..].trim()
        } else if part.contains('=') {
            continue;
        } else {
            part
        };

        if host_port.is_empty() {
            continue;
        }
        if host_port.contains("://") {
            out.push(host_port.to_string());
        } else {
            out.push(format!("http://{host_port}"));
        }
    }
    out
}