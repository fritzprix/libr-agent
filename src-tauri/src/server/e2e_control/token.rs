use std::sync::OnceLock;

pub const E2E_CONTROL_HEADER: &str = "x-libragent-e2e-control";

static CAPABILITY: OnceLock<Option<String>> = OnceLock::new();

/// True when this binary is a debug build and the operator opted in via env.
pub fn e2e_control_enabled() -> bool {
    if !cfg!(debug_assertions) {
        return false;
    }
    std::env::var("LIBRAGENT_E2E_CONTROL").ok().as_deref() == Some("1")
}

fn write_capability_file(token: &str) {
    let Some(base) = capability_file_dir() else {
        return;
    };
    if std::fs::create_dir_all(&base).is_err() {
        return;
    }
    let path = base.join("e2e_control.cap");
    if std::fs::write(&path, token).is_ok() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = std::fs::metadata(&path) {
                let mut perms = meta.permissions();
                perms.set_mode(0o600);
                let _ = std::fs::set_permissions(&path, perms);
            }
        }
    }
}

fn capability_file_dir() -> Option<std::path::PathBuf> {
    if let Ok(dir) = std::env::var("LIBRAGENT_DATA_DIR") {
        if !dir.is_empty() {
            return Some(std::path::PathBuf::from(dir));
        }
    }
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()?;
    Some(
        std::path::PathBuf::from(home)
            .join("AppData")
            .join("Local")
            .join("com.fritzprix.libragent"),
    )
}

/// Mint and store the capability token once per process (debug + opt-in only).
pub fn init_capability_token() {
    if !e2e_control_enabled() {
        let _ = CAPABILITY.set(None);
        return;
    }
    let token = uuid::Uuid::new_v4().to_string().replace('-', "")
        + &uuid::Uuid::new_v4().to_string().replace('-', "");
    write_capability_file(&token);
    let _ = CAPABILITY.set(Some(token));
    log::warn!(
        "E2E control HTTP enabled (loopback, capability auth). Not available in release builds."
    );
}

pub fn capability_token() -> Option<&'static str> {
    CAPABILITY.get().and_then(|v| v.as_deref())
}

/// Harness may pass the same token via env when the capability file is unavailable.
pub fn read_capability_from_env() -> Option<String> {
    std::env::var("LIBRAGENT_E2E_CONTROL_TOKEN")
        .ok()
        .filter(|v| !v.is_empty())
}

pub fn authorize(headers: &warp::http::HeaderMap) -> Result<(), &'static str> {
    if !e2e_control_enabled() {
        return Err("e2e control disabled");
    }
    let expected = capability_token().or_else(|| read_capability_from_env().as_deref());
    let Some(expected) = expected else {
        return Err("e2e control token unavailable");
    };
    let provided = headers
        .get(E2E_CONTROL_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(str::trim);
    match provided {
        Some(value) if value == expected => Ok(()),
        Some(_) => Err("invalid e2e control capability"),
        None => Err("missing e2e control capability header"),
    }
}
