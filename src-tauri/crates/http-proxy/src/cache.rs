use crate::env::{env_http_proxy_for_url, no_proxy_bypasses};
use crate::parse::ResolvedProxy;
use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use url::Url;

#[cfg(windows)]
use crate::windows;

const SUCCESS_TTL: Duration = Duration::from_secs(300);
const FAILURE_TTL: Duration = Duration::from_secs(30);

#[derive(Debug, Clone)]
enum CacheEntry {
    /// Cached proxy URL (`None` = DIRECT).
    Ok { proxy: Option<String>, at: Instant },
    /// Recent resolve failure — stay direct briefly to avoid hammering WinHTTP.
    Err { at: Instant },
}

type ResolveFn = Arc<dyn Fn(&str) -> Result<ResolvedProxy, String> + Send + Sync>;

pub struct Resolver {
    resolve_fn: ResolveFn,
    /// Cache key: scheme + host (+ port) of the request origin.
    by_origin: DashMap<String, CacheEntry>,
}

impl Default for Resolver {
    fn default() -> Self {
        Self {
            resolve_fn: Arc::new(system_resolve_sync),
            by_origin: DashMap::new(),
        }
    }
}

impl Resolver {
    pub fn new() -> Self {
        Self::default()
    }

    /// Test helper: inject a resolve function and start with an empty cache.
    pub fn with_resolve_fn(
        resolve_fn: impl Fn(&str) -> Result<ResolvedProxy, String> + Send + Sync + 'static,
    ) -> Self {
        Self {
            resolve_fn: Arc::new(resolve_fn),
            by_origin: DashMap::new(),
        }
    }

    fn origin_key(url: &str) -> Result<String, String> {
        let parsed = Url::parse(url).map_err(|e| format!("invalid URL for proxy resolve: {e}"))?;
        let host = parsed
            .host_str()
            .ok_or_else(|| "URL has no host for proxy resolve".to_string())?;
        let port = parsed.port_or_known_default().unwrap_or(80);
        Ok(format!("{}://{}:{}", parsed.scheme(), host, port))
    }

    fn entry_fresh(entry: &CacheEntry) -> bool {
        match entry {
            CacheEntry::Ok { at, .. } => at.elapsed() < SUCCESS_TTL,
            CacheEntry::Err { at } => at.elapsed() < FAILURE_TTL,
        }
    }

    /// Pre-resolve and cache. System lookup errors are logged and cached briefly
    /// as direct — callers should not fail the request.
    pub async fn ensure_resolved(&self, url: &str) -> Result<(), String> {
        let key = Self::origin_key(url)?;
        if let Some(entry) = self.by_origin.get(&key) {
            if Self::entry_fresh(entry.value()) {
                return Ok(());
            }
        }
        if no_proxy_bypasses(url) {
            self.by_origin.insert(
                key,
                CacheEntry::Ok {
                    proxy: None,
                    at: Instant::now(),
                },
            );
            return Ok(());
        }
        if Url::parse(url)
            .ok()
            .and_then(|u| env_http_proxy_for_url(&u))
            .is_some()
        {
            self.by_origin.insert(
                key,
                CacheEntry::Ok {
                    proxy: None,
                    at: Instant::now(),
                },
            );
            return Ok(());
        }

        let url_owned = url.to_string();
        let resolve_fn = Arc::clone(&self.resolve_fn);
        let resolved = tokio::task::spawn_blocking(move || resolve_fn(&url_owned))
            .await
            .map_err(|e| format!("proxy resolve task failed: {e}"))?;

        match resolved {
            Ok(proxy) => {
                let proxy_url = proxy.first_reqwest_proxy_url();
                let label = proxy_url.as_deref().unwrap_or("DIRECT");
                log::info!("http-proxy: resolved {label} for {url}");
                self.by_origin.insert(
                    key,
                    CacheEntry::Ok {
                        proxy: proxy_url,
                        at: Instant::now(),
                    },
                );
            }
            Err(err) => {
                log::warn!("http-proxy: resolve failed for {url}: {err}; using direct briefly");
                self.by_origin
                    .insert(key, CacheEntry::Err { at: Instant::now() });
            }
        }
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
        match self.by_origin.get(&key).as_deref() {
            Some(CacheEntry::Ok { proxy, at }) if at.elapsed() < SUCCESS_TTL => proxy.clone(),
            _ => None,
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn caches_failure_briefly_and_does_not_error() {
        let calls = Arc::new(AtomicUsize::new(0));
        let calls2 = calls.clone();
        let resolver = Resolver::with_resolve_fn(move |_| {
            calls2.fetch_add(1, Ordering::SeqCst);
            Err("boom".into())
        });
        resolver
            .ensure_resolved("https://api.example.com/v1")
            .await
            .unwrap();
        resolver
            .ensure_resolved("https://api.example.com/v1")
            .await
            .unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(resolver
            .proxy_for_url(&Url::parse("https://api.example.com/v1").unwrap())
            .is_none());
    }

    #[tokio::test]
    async fn caches_success_proxy() {
        let resolver = Resolver::with_resolve_fn(|_| {
            Ok(ResolvedProxy::Proxies(
                vec!["http://proxy.corp:8080".into()],
            ))
        });
        resolver
            .ensure_resolved("https://api.example.com/v1")
            .await
            .unwrap();
        assert_eq!(
            resolver
                .proxy_for_url(&Url::parse("https://api.example.com/v1").unwrap())
                .as_deref(),
            Some("http://proxy.corp:8080")
        );
    }
}
