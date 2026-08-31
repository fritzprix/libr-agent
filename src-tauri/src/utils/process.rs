use std::io;
use std::process::Command;

/// Preserve useful context when a blocking process-management task fails.
pub(crate) fn describe_join_error(error: tokio::task::JoinError) -> String {
    if error.is_panic() {
        let payload = error.into_panic();
        if let Some(message) = payload.downcast_ref::<String>() {
            return format!("blocking process task panicked: {message}");
        }
        if let Some(message) = payload.downcast_ref::<&'static str>() {
            return format!("blocking process task panicked: {message}");
        }
        return "blocking process task panicked with a non-text payload".to_string();
    }

    if error.is_cancelled() {
        return "blocking process task was cancelled".to_string();
    }

    format!("blocking process task failed: {error}")
}

#[cfg(unix)]
fn process_is_alive(pid: u32) -> io::Result<bool> {
    Ok(Command::new("kill")
        .args(["-0", &pid.to_string()])
        .status()?
        .success())
}

#[cfg(windows)]
fn process_is_alive(pid: u32) -> io::Result<bool> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
        return Ok(false);
    }

    let mut exit_code = 0;
    let is_alive = unsafe { GetExitCodeProcess(handle, &mut exit_code) != 0 && exit_code == 259 };
    unsafe {
        CloseHandle(handle);
    }
    Ok(is_alive)
}

#[cfg(unix)]
fn process_group_id(pid: u32) -> Option<u32> {
    let output = Command::new("ps")
        .args(["-o", "pgid=", "-p", &pid.to_string()])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout).trim().parse().ok()
}

/// Kill a process and its descendants when the process is its own group leader.
///
/// A negative PID is only passed to `kill` after confirming that the process
/// group ID equals the target PID. This preserves tree-kill behavior for
/// isolated commands while avoiding collateral damage to an inherited group.
///
/// On Windows, `taskkill /T` includes descendants in the target process tree.
/// Callers must provide a session-owned PID; detached processes outside that
/// tree are not guaranteed to be reachable by this function.
pub(crate) fn force_kill_process_tree(pid: u32) -> io::Result<()> {
    #[cfg(unix)]
    if pid <= 1 {
        return Err(io::Error::other(format!(
            "refusing to kill protected process {pid}"
        )));
    }

    #[cfg(windows)]
    if pid <= 4 {
        return Ok(());
    }

    if !process_is_alive(pid)? {
        return Ok(());
    }

    #[cfg(unix)]
    {
        let target = match process_group_id(pid) {
            Some(group_id) if group_id == pid => format!("-{pid}"),
            _ => pid.to_string(),
        };
        let status = Command::new("kill").args(["-KILL", &target]).status()?;
        if status.success() || !process_is_alive(pid)? {
            return Ok(());
        }
        Err(io::Error::other(format!(
            "failed to kill process {pid}: {status}"
        )))
    }

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;

        let mut command = Command::new("taskkill");
        command.args(["/PID", &pid.to_string(), "/T", "/F"]);
        crate::utils::env::apply_isolated_env(&mut command);
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
        let output = command.output()?;
        if output.status.success() || !process_is_alive(pid)? {
            return Ok(());
        }
        Err(io::Error::other(format!(
            "failed to kill process {pid}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )))
    }
}

/// Kill only the supplied process.
///
/// Persistent shells are not allowed to rely on process-group ownership during
/// cleanup because they may have been created by a caller with an inherited
/// process group. Killing a negative PID in that situation can terminate the
/// caller, the test runner, or the desktop session.
pub(crate) fn force_kill_process(pid: u32) -> io::Result<()> {
    #[cfg(unix)]
    if pid <= 1 {
        return Err(io::Error::other(format!(
            "refusing to kill protected process {pid}"
        )));
    }

    #[cfg(windows)]
    if pid <= 4 {
        return Err(io::Error::other(format!(
            "refusing to kill protected process {pid}"
        )));
    }

    if !process_is_alive(pid)? {
        return Ok(());
    }

    #[cfg(unix)]
    {
        let status = Command::new("kill")
            .args(["-KILL", &pid.to_string()])
            .status()?;
        if status.success() || !process_is_alive(pid)? {
            return Ok(());
        }
        Err(io::Error::other(format!(
            "failed to kill process {pid}: {status}"
        )))
    }

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;

        let mut command = Command::new("taskkill");
        command.args(["/PID", &pid.to_string(), "/F"]);
        crate::utils::env::apply_isolated_env(&mut command);
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
        let output = command.output()?;
        if output.status.success() || !process_is_alive(pid)? {
            return Ok(());
        }
        Err(io::Error::other(format!(
            "failed to kill process {pid}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )))
    }
}
