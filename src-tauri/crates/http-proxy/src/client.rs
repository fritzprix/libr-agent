use crate::cache::Resolver;
use std::sync::Arc;

/// Configure a reqwest builder with Windows PAC/system proxy resolution.
///
/// On non-Windows platforms this is a no-op so reqwest's normal env /
/// `system-proxy` behavior stays unchanged (#2846).
pub fn configure_reqwest_builder(
    builder: reqwest::ClientBuilder,
    resolver: Arc<Resolver>,
) -> reqwest::Result<reqwest::ClientBuilder> {
    #[cfg(windows)]
    {
        use reqwest::Proxy;
        let proxy = Proxy::custom(move |url| resolver.proxy_for_url(url));
        Ok(builder.proxy(proxy))
    }
    #[cfg(not(windows))]
    {
        let _ = resolver;
        Ok(builder)
    }
}
