use crate::proxy_types::{ResolvedProxy, SystemProxyDiscovery};
use std::env;

fn env_option(key: &str) -> Option<String> {
    env::var(key).ok().filter(|v| !v.trim().is_empty())
}

pub fn discover() -> Result<SystemProxyDiscovery, String> {
    Ok(SystemProxyDiscovery {
        auto_detect: None,
        pac_url: env_option("PAC_URL"),
        static_proxy: env_option("HTTPS_PROXY")
            .or_else(|| env_option("HTTP_PROXY"))
            .or_else(|| env_option("ALL_PROXY")),
        bypass_list: env_option("NO_PROXY"),
        notes: vec![
            "Source: process environment (HTTP_PROXY / HTTPS_PROXY / NO_PROXY)".into(),
            "PAC is not evaluated on this platform in the PoC; set HTTPS_PROXY manually to test proxied mode.".into(),
        ],
    })
}

pub fn resolve(url: &str) -> Result<ResolvedProxy, String> {
    let bypass = env_option("NO_PROXY");
    if let Some(list) = &bypass {
        for entry in list.split([',', ';']) {
            let entry = entry.trim();
            if entry.is_empty() {
                continue;
            }
            if entry == "*" {
                return Ok(ResolvedProxy::Direct);
            }
            if url.contains(entry.trim_start_matches('*')) {
                return Ok(ResolvedProxy::Direct);
            }
        }
    }

    let proxy = if url.starts_with("https://") {
        env_option("HTTPS_PROXY").or_else(|| env_option("ALL_PROXY"))
    } else {
        env_option("HTTP_PROXY")
            .or_else(|| env_option("HTTPS_PROXY"))
            .or_else(|| env_option("ALL_PROXY"))
    };

    match proxy {
        Some(p) => {
            let normalized = if p.contains("://") {
                p
            } else {
                format!("http://{p}")
            };
            Ok(ResolvedProxy::Proxies(vec![normalized]))
        }
        None => Ok(ResolvedProxy::Direct),
    }
}