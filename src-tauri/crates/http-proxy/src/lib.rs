//! Resolve HTTP proxies for reqwest on Windows (PAC via WinHTTP) while honoring
//! `HTTP_PROXY` / `HTTPS_PROXY` / `NO_PROXY`. Other platforms keep reqwest's
//! normal env / system-proxy behavior (this crate's custom proxy is Windows-only).

mod cache;
mod client;
mod env;
mod parse;
#[cfg(windows)]
mod windows;

pub use client::configure_reqwest_builder;
pub use parse::{parse_proxy_list, parse_static_proxy_for_scheme, proxy_bypass_matches, ResolvedProxy};

use std::sync::{Arc, LazyLock};
use url::Url;

static RESOLVER: LazyLock<Arc<cache::Resolver>> =
    LazyLock::new(|| Arc::new(cache::Resolver::new()));

/// Apply proxy settings to a [`reqwest::ClientBuilder`] (custom proxy + env + WinHTTP on Windows).
pub fn configure_client_builder(
    builder: reqwest::ClientBuilder,
) -> reqwest::Result<reqwest::ClientBuilder> {
    configure_reqwest_builder(builder, Arc::clone(&RESOLVER))
}

/// Pre-resolve the proxy for `url` on a blocking thread (WinHTTP on Windows). Call
/// this from async code before the first request to a new origin so the sync
/// reqwest proxy callback never blocks the runtime.
///
/// System lookup failures are logged and treated as direct (Ok); they do not
/// fail the caller.
pub async fn ensure_proxy_resolved(url: &str) -> Result<(), String> {
    RESOLVER.ensure_resolved(url).await
}

/// Synchronous proxy pick used by reqwest's custom proxy (cache must already be warm).
#[doc(hidden)]
pub fn proxy_url_for_reqwest(url: &Url) -> Option<String> {
    RESOLVER.proxy_for_url(url)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env::{env_http_proxy_for_url, no_proxy_bypasses};
    use crate::parse::parse_proxy_list;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn parses_pac_style_proxy_list() {
        let list = parse_proxy_list("PROXY proxy.corp:8080; DIRECT");
        assert_eq!(list, vec!["http://proxy.corp:8080".to_string()]);

        let list = parse_proxy_list("PROXY a:8080; PROXY b:9090; DIRECT");
        assert_eq!(
            list,
            vec![
                "http://a:8080".to_string(),
                "http://b:9090".to_string(),
            ]
        );
    }

    #[test]
    fn parses_socks_and_bare_host() {
        let list = parse_proxy_list("SOCKS socks.corp:1080; PROXY http.corp:3128");
        assert_eq!(
            list,
            vec![
                "socks5://socks.corp:1080".to_string(),
                "http://http.corp:3128".to_string(),
            ]
        );
        assert_eq!(
            parse_static_proxy_for_scheme("HTTP=legacy:8888", "http"),
            ResolvedProxy::Proxies(vec!["http://legacy:8888".into()])
        );
    }

    #[test]
    fn bypass_rules() {
        let _g = ENV_LOCK.lock().unwrap();
        std::env::set_var("NO_PROXY", "localhost,*.internal,10.0.0.1");
        assert!(no_proxy_bypasses("https://api.internal/v1"));
        assert!(no_proxy_bypasses("http://localhost:8080/"));
        assert!(no_proxy_bypasses("https://10.0.0.1/foo"));
        assert!(!no_proxy_bypasses("https://api.openai.com/v1"));
        assert!(!no_proxy_bypasses("https://api.openai.com/v1/internal/x"));
        std::env::remove_var("NO_PROXY");
    }

    #[test]
    fn env_precedence_over_system_cache() {
        let _g = ENV_LOCK.lock().unwrap();
        std::env::set_var("HTTP_PROXY", "http://env-proxy:9999");
        let url = Url::parse("http://example.com/path").unwrap();
        let picked = env_http_proxy_for_url(&url);
        assert_eq!(picked.as_deref(), Some("http://env-proxy:9999"));
        std::env::remove_var("HTTP_PROXY");
    }

    #[test]
    fn empty_and_direct_tokens_in_proxy_list() {
        assert!(parse_proxy_list("").is_empty());
        assert!(parse_proxy_list("DIRECT").is_empty());
        assert_eq!(
            parse_proxy_list("PROXY only:1; DIRECT"),
            vec!["http://only:1".to_string()]
        );
    }

    #[test]
    fn https_proxy_env_for_tls_urls() {
        let _g = ENV_LOCK.lock().unwrap();
        std::env::remove_var("HTTP_PROXY");
        std::env::remove_var("http_proxy");
        std::env::set_var("HTTPS_PROXY", "https://secure-proxy:443");
        let url = Url::parse("https://api.example.com/v1").unwrap();
        assert_eq!(
            env_http_proxy_for_url(&url).as_deref(),
            Some("https://secure-proxy:443")
        );
        std::env::remove_var("HTTPS_PROXY");
    }

    #[test]
    fn resolved_proxy_first_url() {
        let r = ResolvedProxy::Proxies(vec!["http://p:1".into()]);
        assert_eq!(r.first_reqwest_proxy_url().as_deref(), Some("http://p:1"));
        assert!(ResolvedProxy::Direct.first_reqwest_proxy_url().is_none());
    }
}
