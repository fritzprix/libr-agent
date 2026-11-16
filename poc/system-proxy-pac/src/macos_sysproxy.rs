use crate::proxy_types::{ResolvedProxy, SystemProxyDiscovery};
use core_foundation::array::CFArray;
use core_foundation::base::{CFRelease, TCFType};
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use core_foundation::url::CFURL;
use core_foundation_sys::dictionary::CFDictionaryGetValue;
use core_foundation_sys::string::CFStringRef;
use std::ffi::c_void;

#[link(name = "CFNetwork", kind = "framework")]
extern "C" {
    fn CFNetworkCopyProxiesForURL(url: *const c_void, proxy_settings: *const c_void) -> *const c_void;
    fn CFNetworkCopySystemProxySettings() -> *const c_void;
}

extern "C" {
    #[link_name = "kCFProxyTypeKey"]
    static K_CF_PROXY_TYPE_KEY: CFStringRef;
    #[link_name = "kCFProxyHostNameKey"]
    static K_CF_PROXY_HOST_NAME_KEY: CFStringRef;
    #[link_name = "kCFProxyPortNumberKey"]
    static K_CF_PROXY_PORT_NUMBER_KEY: CFStringRef;
    #[link_name = "kCFProxyTypeNone"]
    static K_CF_PROXY_TYPE_NONE: CFStringRef;
}

fn dict_string(dict: &CFDictionary, key: CFStringRef) -> Option<String> {
    unsafe {
        let value = CFDictionaryGetValue(dict.as_concrete_TypeRef(), key as *const c_void);
        if value.is_null() {
            return None;
        }
        CFString::wrap_under_get_rule(value as _).to_string()
    }
}

fn dict_port(dict: &CFDictionary, key: CFStringRef) -> Option<u16> {
    unsafe {
        let value = CFDictionaryGetValue(dict.as_concrete_TypeRef(), key as *const c_void);
        if value.is_null() {
            return None;
        }
        CFNumber::wrap_under_get_rule(value as _)
            .to_i64()
            .and_then(|v| u16::try_from(v).ok())
    }
}

pub fn discover() -> Result<SystemProxyDiscovery, String> {
    let store = system_configuration::dynamic_store::SCDynamicStoreBuilder::new("system-proxy-pac")
        .build();

    let mut discovery = SystemProxyDiscovery {
        notes: vec!["Source: SCDynamicStoreCopyProxies + CFNetworkCopySystemProxySettings".into()],
        ..Default::default()
    };

    if let Some(dict) = store.get_proxies_global() {
        if let Some(url) = dict.get("ProxyAutoConfigURLString") {
            discovery.pac_url = Some(url.clone());
        }
        if dict.get("ProxyAutoDiscoveryEnable").copied() == Some(1) {
            discovery.auto_detect = Some(true);
        }
        if let Some(http) = dict.get("HTTPProxy") {
            let port = dict.get("HTTPPort").cloned().unwrap_or_default();
            discovery.static_proxy = Some(format!("{http}:{port}"));
        }
        if let Some(bypass) = dict.get("ExceptionsList") {
            discovery.bypass_list = Some(bypass.clone());
        }
    }

    Ok(discovery)
}

pub fn resolve(url: &str) -> Result<ResolvedProxy, String> {
    let target =
        CFURL::from_url(url, false).map_err(|e| format!("Invalid target URL: {e:?}"))?;

    unsafe {
        let settings = CFNetworkCopySystemProxySettings();
        if settings.is_null() {
            return Ok(ResolvedProxy::Direct);
        }

        let proxies_ref =
            CFNetworkCopyProxiesForURL(target.as_concrete_TypeRef() as _, settings);
        CFRelease(settings);

        if proxies_ref.is_null() {
            return Ok(ResolvedProxy::Direct);
        }

        let proxies = CFArray::<CFDictionary>::wrap_under_create_rule(proxies_ref as _);
        let mut resolved = Vec::new();

        for entry in proxies.iter() {
            let ptype = dict_string(entry, K_CF_PROXY_TYPE_KEY);
            if ptype.as_deref() == Some("kCFProxyTypeNone") {
                continue;
            }
            if let Some(none) = dict_string(entry, K_CF_PROXY_TYPE_NONE) {
                let _ = none;
            }
            unsafe {
                let raw_type = CFDictionaryGetValue(
                    entry.as_concrete_TypeRef(),
                    K_CF_PROXY_TYPE_KEY as *const c_void,
                );
                if raw_type == K_CF_PROXY_TYPE_NONE as *const c_void {
                    continue;
                }
            }

            if let (Some(host), Some(port)) = (
                dict_string(entry, K_CF_PROXY_HOST_NAME_KEY),
                dict_port(entry, K_CF_PROXY_PORT_NUMBER_KEY),
            ) {
                resolved.push(format!("http://{host}:{port}"));
                break;
            }
        }

        if resolved.is_empty() {
            Ok(ResolvedProxy::Direct)
        } else {
            Ok(ResolvedProxy::Proxies(resolved))
        }
    }
}