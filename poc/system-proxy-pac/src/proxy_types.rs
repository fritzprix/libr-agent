/// Raw proxy settings discovered from the OS (or environment on unsupported platforms).
#[derive(Debug, Clone, Default)]
pub struct SystemProxyDiscovery {
    pub auto_detect: Option<bool>,
    pub pac_url: Option<String>,
    pub static_proxy: Option<String>,
    pub bypass_list: Option<String>,
    pub notes: Vec<String>,
}

/// Proxy chain resolved for a specific target URL.
#[derive(Debug, Clone)]
pub enum ResolvedProxy {
    Direct,
    Proxies(Vec<String>),
}

impl ResolvedProxy {
    pub fn display_label(&self) -> String {
        match self {
            ResolvedProxy::Direct => "DIRECT".to_string(),
            ResolvedProxy::Proxies(list) if list.is_empty() => "DIRECT".to_string(),
            ResolvedProxy::Proxies(list) => list.join(", "),
        }
    }

    /// First HTTP proxy URL suitable for reqwest (`http://host:port`).
    pub fn first_reqwest_proxy_url(&self) -> Option<String> {
        let ResolvedProxy::Proxies(list) = self else {
            return None;
        };
        list.first().cloned()
    }
}

/// Parse WinHTTP / PAC-style proxy list strings into reqwest-ready HTTP proxy URLs.
#[cfg_attr(not(windows), allow(dead_code))]
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
            out.push(format!("socks5://{hp}"));
            continue;
        } else if let Some(rest) = upper.strip_prefix("HTTP=") {
            rest.trim()
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