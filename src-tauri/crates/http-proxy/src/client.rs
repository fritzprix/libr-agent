use crate::cache::Resolver;
use reqwest::Proxy;
use std::sync::Arc;

/// Configure a reqwest builder with env + Windows PAC/system proxy resolution.
///
/// Does not call [`reqwest::ClientBuilder::no_proxy`] afterward (that would clear proxies).
pub fn configure_reqwest_builder(
    mut builder: reqwest::ClientBuilder,
    resolver: Arc<Resolver>,
) -> reqwest::Result<reqwest::ClientBuilder> {
    let proxy = Proxy::custom(move |url| resolver.proxy_for_url(url));
    builder = builder.proxy(proxy);
    Ok(builder)
}