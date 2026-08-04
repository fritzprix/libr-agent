//! Recover the full Windows PATH from the registry.
//!
//! GUI-launched processes (Explorer, Start menu, some installers) often inherit a
//! stripped process PATH that omits user-local tool directories still present in
//! `HKCU\Environment\Path`. Isolated child processes clear the host env and rebuild
//! PATH via [`crate::utils::env::get_effective_path`]; without a registry probe those
//! directories are permanently lost (unlike Unix, where a login-shell PATH probe runs).
//!
//! Construction order matches Windows: Machine Path, then User Path.

use std::ffi::{OsStr, OsString};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::sync::OnceLock;

use windows_sys::Win32::System::Environment::ExpandEnvironmentStringsW;
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, REG_EXPAND_SZ, REG_SZ};
use winreg::RegKey;

const MACHINE_ENV_SUBKEY: &str = r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment";
const USER_ENV_SUBKEY: &str = r"Environment";

/// Cached Machine+User PATH from the Windows registry (expanded).
pub fn get_windows_registry_path_os() -> Option<OsString> {
    static CACHED: OnceLock<Option<OsString>> = OnceLock::new();
    CACHED.get_or_init(read_windows_registry_path).clone()
}

fn read_windows_registry_path() -> Option<OsString> {
    let machine = read_registry_path_value(HKEY_LOCAL_MACHINE, MACHINE_ENV_SUBKEY);
    let user = read_registry_path_value(HKEY_CURRENT_USER, USER_ENV_SUBKEY);

    match (machine, user) {
        (Some(machine_path), Some(user_path)) => {
            merge_path_os(machine_path.as_os_str(), user_path.as_os_str())
        }
        (Some(machine_path), None) => Some(machine_path),
        (None, Some(user_path)) => Some(user_path),
        (None, None) => None,
    }
}

fn read_registry_path_value(hive: winreg::HKEY, subkey: &str) -> Option<OsString> {
    let root = RegKey::predef(hive);
    let key = root.open_subkey(subkey).ok()?;
    let raw = key.get_raw_value("Path").ok()?;
    let stored = raw.to_string();
    if stored.trim().is_empty() {
        return None;
    }

    match raw.vtype {
        REG_EXPAND_SZ => expand_environment_strings(&stored),
        REG_SZ => Some(OsString::from(stored)),
        _ => Some(OsString::from(stored)),
    }
}

fn expand_environment_strings(input: &str) -> Option<OsString> {
    let wide: Vec<u16> = OsStr::new(input)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    // First call queries required length (chars including NUL).
    // SAFETY: `wide` is a valid NUL-terminated UTF-16 buffer owned by this stack frame.
    // Passing a null destination with nSize=0 is the documented size-query pattern for
    // ExpandEnvironmentStringsW; the API does not write through a null lpDst in that case.
    let required = unsafe { ExpandEnvironmentStringsW(wide.as_ptr(), std::ptr::null_mut(), 0) };
    if required == 0 {
        return Some(OsString::from(input));
    }

    let mut buffer = vec![0u16; required as usize];
    // SAFETY: `wide` remains a valid NUL-terminated source; `buffer` has exactly `required`
    // UTF-16 slots as reported by the size-query call above. ExpandEnvironmentStringsW is
    // safe to call concurrently for distinct buffers.
    let written =
        unsafe { ExpandEnvironmentStringsW(wide.as_ptr(), buffer.as_mut_ptr(), required) };
    if written == 0 || written > required {
        return Some(OsString::from(input));
    }

    // written includes the terminating NUL.
    let len = (written as usize).saturating_sub(1);
    Some(OsString::from_wide(&buffer[..len]))
}

fn merge_path_os(first: &OsStr, second: &OsStr) -> Option<OsString> {
    let mut merged = Vec::new();

    for source in [first, second] {
        for path in std::env::split_paths(source) {
            if path.as_os_str().is_empty() || merged.iter().any(|existing| existing == &path) {
                continue;
            }
            merged.push(path);
        }
    }

    if merged.is_empty() {
        None
    } else {
        std::env::join_paths(merged).ok()
    }
}
