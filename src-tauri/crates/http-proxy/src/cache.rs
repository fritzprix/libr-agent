use crate::env::{env_http_proxy_for_url, no_proxy_bypasses};
use crate::parse::ResolvedProxy;
use dashmap::DashMap;
use url::Url;

#[cfg(windows)]
use crate::windows;

pub struct Resolver {
    /// Cache key: scheme + host (+ port) of the request origin.
    by_origin: DashMap<String, Option<String>>,
}

impl Default for Resolver {
    fn default() -> Self {
        Self {
            by_origin: DashMap::new(),
        }
    }
}

impl Resolver {
    pub fn new() -> Self {
        Self::default()
    }

    fn origin_key(url: &str) -> Result<String, String> {
        let parsed = Url::parse(url).map_err(|e| format!("invalid URL for proxy resolve: {e}"))?;
        let host = parsed
            .host_str()
            .ok_or_else(|| "URL has no host for proxy resolve".to_string())?;
        let port = parsed.port_or_known_default().unwrap_or(80);
        Ok(format!("{}://{}:{}", parsed.scheme(), host, port))
    }

    pub async fn ensure_resolved(&self, url: &str) -> Result<(), String> {
        let key = Self::origin_key(url)?;
        if self.by_origin.contains_key(&key) {
            return Ok(());
        }
        if no_proxy_bypasses(url) {
            self.by_origin.insert(key, None);
            return Ok(());
        }
        if env_http_proxy_for_url(&Url::parse(url).unwrap()).is_some() {
            // Env is read at request time; no WinHTTP work needed.
            self.by_origin.insert(key, None);
            return Ok(());
        }

        let url_owned = url.to_string();
        let resolved = tokio::task::spawn_blocking(move || system_resolve_sync(&url_owned))
            .await
            .map_err(|e| format!("proxy resolve task failed: {e}"))??;

        let proxy_url = resolved.first_reqwest_proxy_url();
        let label = proxy_url.as_deref().unwrap_or("DIRECT");
        log::info!("http-proxy: resolved {label} for {url}");
        log::debug!("http-proxy: cached {label} for origin {key}");
        self.by_origin.insert(key, proxy_url);
        Ok(())
    }

    pub fn proxy_for_url(&self, url: &Url) -> Option<String> {
        if no_proxy_bypasses(url.as_str()) {
            return None;
        }
        if let Some(env_proxy) = env_http_proxy_for_url(url) {
            return Some(env_proxy);
        }
        let key = format!(
            "{}://{}:{}",
            url.scheme(),
            url.host_str()?,
            url.port_or_known_default().unwrap_or(80)
        );
        self.by_origin
            .get(&key)
            .and_then(|entry| entry.value().clone())
    }
}

#[cfg(windows)]
fn system_resolve_sync(url: &str) -> Result<ResolvedProxy, String> {
    windows::resolve(url)
}

#[cfg(not(windows))]
fn system_resolve_sync(_url: &str) -> Result<ResolvedProxy, String> {
    Ok(ResolvedProxy::Direct)
}