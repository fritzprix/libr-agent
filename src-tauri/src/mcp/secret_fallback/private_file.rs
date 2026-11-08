//! Creating files readable and writable by the current user only.

use std::io::Write;
use std::path::Path;

/// Create `path` readable and writable by the current user only (Unix 0600; on Windows a
/// protected ACL that grants the current user's SID), then write `bytes`.
pub(super) fn write_private_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|e| format!("Failed to create {}: {e}", path.display()))?;
    #[cfg(windows)]
    restrict_to_current_user(path)?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|e| format!("Failed to write {}: {e}", path.display()))
}

/// The current process user's SID in string form (for example `S-1-5-21-...`).
#[cfg(windows)]
fn current_user_sid() -> Result<String, String> {
    use windows_sys::Win32::Foundation::{CloseHandle, LocalFree, HANDLE};
    use windows_sys::Win32::Security::Authorization::ConvertSidToStringSidW;
    use windows_sys::Win32::Security::{GetTokenInformation, TokenUser, TOKEN_QUERY, TOKEN_USER};
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    let os_error = |what: &str| format!("{what}: {}", std::io::Error::last_os_error());
    // SAFETY: the token handle is closed once on every path after it is opened. The
    // TOKEN_USER buffer is u64-aligned and as large as GetTokenInformation asked for, and
    // the SID string allocated by ConvertSidToStringSidW is read up to its NUL and freed
    // once with LocalFree.
    unsafe {
        let mut token: HANDLE = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return Err(os_error("Failed to open the process token"));
        }
        let mut needed = 0u32;
        GetTokenInformation(token, TokenUser, std::ptr::null_mut(), 0, &mut needed);
        let mut buffer = vec![0u64; (needed as usize).div_ceil(8)];
        let read = GetTokenInformation(
            token,
            TokenUser,
            buffer.as_mut_ptr().cast(),
            needed,
            &mut needed,
        );
        let read_error = os_error("Failed to read the process user");
        CloseHandle(token);
        if read == 0 {
            return Err(read_error);
        }
        let user = &*buffer.as_ptr().cast::<TOKEN_USER>();
        let mut sid_string: *mut u16 = std::ptr::null_mut();
        if ConvertSidToStringSidW(user.User.Sid, &mut sid_string) == 0 {
            return Err(os_error("Failed to format the process user SID"));
        }
        let len = (0..).take_while(|&i| *sid_string.add(i) != 0).count();
        let sid = String::from_utf16_lossy(std::slice::from_raw_parts(sid_string, len));
        LocalFree(sid_string.cast());
        Ok(sid)
    }
}

#[cfg(windows)]
fn restrict_to_current_user(path: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Authorization::{
        ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
    };
    use windows_sys::Win32::Security::{
        SetFileSecurityW, DACL_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION,
        PSECURITY_DESCRIPTOR,
    };

    // Protected DACL: full access for the current user's SID, nothing inherited. The owner
    // alias (OW) is not used: under an elevated token the owner is Administrators, and a
    // later non-elevated run would lose access to the key.
    let sddl: Vec<u16> = format!("D:P(A;;FA;;;{})", current_user_sid()?)
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let wide_path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut descriptor: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
    // SAFETY: both strings are NUL-terminated UTF-16 buffers that outlive the calls, and
    // the descriptor allocated by the first call is freed once with LocalFree.
    unsafe {
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            SDDL_REVISION_1,
            &mut descriptor,
            std::ptr::null_mut(),
        ) == 0
        {
            return Err(format!(
                "Failed to build the key file ACL: {}",
                std::io::Error::last_os_error()
            ));
        }
        let applied = SetFileSecurityW(
            wide_path.as_ptr(),
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            descriptor,
        );
        let error = std::io::Error::last_os_error();
        LocalFree(descriptor);
        if applied == 0 {
            return Err(format!(
                "Failed to restrict {} to the current user: {error}",
                path.display()
            ));
        }
    }
    Ok(())
}
