use crate::parse::{
    parse_proxy_list, parse_static_proxy_for_scheme, proxy_bypass_matches, ResolvedProxy,
};
use std::ffi::c_void;
use url::Url;
use windows::core::{HSTRING, PCWSTR, PWSTR};
use windows::Win32::Foundation::{GetLastError, GlobalFree, HGLOBAL};
use windows::Win32::Networking::WinHttp::{
    WinHttpCloseHandle, WinHttpGetIEProxyConfigForCurrentUser, WinHttpGetProxyForUrl, WinHttpOpen,
    WINHTTP_ACCESS_TYPE_NAMED_PROXY, WINHTTP_ACCESS_TYPE_NO_PROXY, WINHTTP_AUTOPROXY_AUTO_DETECT,
    WINHTTP_AUTOPROXY_CONFIG_URL, WINHTTP_AUTOPROXY_OPTIONS, WINHTTP_AUTO_DETECT_TYPE_DHCP,
    WINHTTP_AUTO_DETECT_TYPE_DNS_A, WINHTTP_CURRENT_USER_IE_PROXY_CONFIG, WINHTTP_PROXY_INFO,
};

fn pwstr_to_string(ptr: PWSTR) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    unsafe { PCWSTR(ptr.0).to_string().ok() }
}

fn free_pwstr(ptr: PWSTR) {
    if !ptr.is_null() {
        unsafe {
            let _ = GlobalFree(HGLOBAL(ptr.0 as *mut c_void));
        }
    }
}

fn discover_ie_config() -> Result<WINHTTP_CURRENT_USER_IE_PROXY_CONFIG, String> {
    let mut config = WINHTTP_CURRENT_USER_IE_PROXY_CONFIG::default();
    unsafe {
        WinHttpGetIEProxyConfigForCurrentUser(&mut config).map_err(|_| {
            format!(
                "WinHttpGetIEProxyConfigForCurrentUser failed: {:?}",
                GetLastError()
            )
        })?;
    }
    Ok(config)
}

fn request_scheme(url: &str) -> &str {
    Url::parse(url)
        .ok()
        .map(|u| {
            // Keep the lifetime of the static scheme literals only; fall back below.
            match u.scheme() {
                "https" => "https",
                "http" => "http",
                _ => "http",
            }
        })
        .unwrap_or("http")
}

/// Resolve the system proxy for `url` via WinHTTP (PAC / auto-detect / static).
pub fn resolve(url: &str) -> Result<ResolvedProxy, String> {
    let config = discover_ie_config()?;

    let auto_detect = config.fAutoDetect.as_bool();
    let pac_url_s = pwstr_to_string(config.lpszAutoConfigUrl);
    let static_proxy_s = pwstr_to_string(config.lpszProxy);
    let bypass_s = pwstr_to_string(config.lpszProxyBypass);

    if let Some(ref bypass) = bypass_s {
        if proxy_bypass_matches(url, bypass) {
            free_ie_config_strings(&config);
            return Ok(ResolvedProxy::Direct);
        }
    }

    let session = open_session()?;

    let resolved = if auto_detect || pac_url_s.is_some() {
        // Corporate PAC files often require NTLM for the PAC fetch itself.
        let mut options = WINHTTP_AUTOPROXY_OPTIONS {
            fAutoLogonIfChallenged: windows::Win32::Foundation::TRUE,
            ..Default::default()
        };
        if auto_detect {
            options.dwFlags |= WINHTTP_AUTOPROXY_AUTO_DETECT;
            options.dwAutoDetectFlags =
                WINHTTP_AUTO_DETECT_TYPE_DHCP | WINHTTP_AUTO_DETECT_TYPE_DNS_A;
        }

        let pac_h = pac_url_s.as_ref().map(|pac| HSTRING::from(pac.as_str()));

        if let Some(ref pac) = pac_h {
            options.dwFlags |= WINHTTP_AUTOPROXY_CONFIG_URL;
            options.lpszAutoConfigUrl = PCWSTR(pac.as_ptr());
        }

        let url_h = HSTRING::from(url);
        let mut proxy_info = WINHTTP_PROXY_INFO::default();
        let proxy_result = unsafe {
            WinHttpGetProxyForUrl(
                session,
                PCWSTR(url_h.as_ptr()),
                &mut options,
                &mut proxy_info,
            )
        };

        free_ie_config_strings(&config);
        close_session(session);

        if let Err(err) = proxy_result {
            return Err(format!(
                "WinHttpGetProxyForUrl failed: {err:?} (last error {:?})",
                unsafe { GetLastError() }
            ));
        }

        let result = proxy_info_to_resolved(&proxy_info)?;
        free_proxy_info(&mut proxy_info);
        result
    } else if let Some(ref raw) = static_proxy_s {
        free_ie_config_strings(&config);
        close_session(session);
        parse_static_proxy_for_scheme(raw, request_scheme(url))
    } else {
        free_ie_config_strings(&config);
        close_session(session);
        ResolvedProxy::Direct
    };

    Ok(resolved)
}

fn open_session() -> Result<*mut c_void, String> {
    let session = unsafe {
        WinHttpOpen(
            PCWSTR::null(),
            WINHTTP_ACCESS_TYPE_NO_PROXY,
            PCWSTR::null(),
            PCWSTR::null(),
            0,
        )
    };
    if session.is_null() {
        Err(format!("WinHttpOpen failed: {:?}", unsafe {
            GetLastError()
        }))
    } else {
        Ok(session)
    }
}

fn close_session(session: *mut c_void) {
    unsafe {
        let _ = WinHttpCloseHandle(session);
    }
}

fn free_ie_config_strings(config: &WINHTTP_CURRENT_USER_IE_PROXY_CONFIG) {
    free_pwstr(config.lpszAutoConfigUrl);
    free_pwstr(config.lpszProxy);
    free_pwstr(config.lpszProxyBypass);
}

fn proxy_info_to_resolved(info: &WINHTTP_PROXY_INFO) -> Result<ResolvedProxy, String> {
    match info.dwAccessType {
        WINHTTP_ACCESS_TYPE_NO_PROXY => Ok(ResolvedProxy::Direct),
        WINHTTP_ACCESS_TYPE_NAMED_PROXY => {
            let proxy = pwstr_to_string(info.lpszProxy).unwrap_or_default();
            let proxies = parse_proxy_list(&proxy);
            if proxies.is_empty() {
                Ok(ResolvedProxy::Direct)
            } else {
                Ok(ResolvedProxy::Proxies(proxies))
            }
        }
        other => Err(format!("unexpected WINHTTP proxy access type: {other:?}")),
    }
}

fn free_proxy_info(info: &mut WINHTTP_PROXY_INFO) {
    free_pwstr(info.lpszProxy);
    free_pwstr(info.lpszProxyBypass);
    info.lpszProxy = PWSTR::null();
    info.lpszProxyBypass = PWSTR::null();
}
