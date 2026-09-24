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

/// Collect every descendant of `root` by walking `/proc/*/status` PPID links.
///
/// Chromium sandbox helpers often call `setpgid`/`setsid`, so killing only the
/// sidecar process group leaves orphaned browser processes that keep holding
/// the automation session open. PPID ancestry still reaches those children.
#[cfg(unix)]
fn collect_descendant_pids(root: u32) -> Vec<u32> {
    use std::collections::{HashMap, HashSet, VecDeque};
    use std::fs;

    let mut children_by_ppid: HashMap<u32, Vec<u32>> = HashMap::new();
    let Ok(entries) = fs::read_dir("/proc") else {
        return Vec::new();
    };

    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(pid) = name.to_str().and_then(|s| s.parse::<u32>().ok()) else {
            continue;
        };
        if pid <= 1 {
            continue;
        }
        let status_path = entry.path().join("status");
        let Ok(status) = fs::read_to_string(status_path) else {
            continue;
        };
        let Some(ppid_line) = status.lines().find(|line| line.starts_with("PPid:")) else {
            continue;
        };
        let Some(ppid) = ppid_line
            .split_whitespace()
            .nth(1)
            .and_then(|value| value.parse::<u32>().ok())
        else {
            continue;
        };
        children_by_ppid.entry(ppid).or_default().push(pid);
    }

    let mut ordered = Vec::new();
    let mut seen = HashSet::from([root]);
    let mut queue = VecDeque::from([root]);
    while let Some(current) = queue.pop_front() {
        let Some(children) = children_by_ppid.get(&current) else {
            continue;
        };
        for &child in children {
            if seen.insert(child) {
                ordered.push(child);
                queue.push_back(child);
            }
        }
    }
    ordered
}

#[cfg(unix)]
fn signal_kill(pid_or_group: &str) -> io::Result<std::process::ExitStatus> {
    Command::new("kill").args(["-KILL", pid_or_group]).status()
}

/// Kill a process and its descendants when the process is its own group leader.
///
/// A negative PID is only passed to `kill` after confirming that the process
/// group ID equals the target PID. This preserves tree-kill behavior for
/// isolated commands while avoiding collateral damage to an inherited group.
///
/// On Unix, after the process-group signal we also walk `/proc` PPID ancestry and
/// SIGKILL any remaining descendants. Chromium often leaves the original process
/// group; group-only kill then leaves the browser alive while the sidecar parent
/// is already gone.
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
        // Snapshot descendants before signalling so we still know what to reap
        // after the root/group disappears.
        let descendants = collect_descendant_pids(pid);

        if let Some(group_id) = process_group_id(pid) {
            if group_id == pid {
                let _ = signal_kill(&format!("-{pid}"));
            }
        }

        // Children first, then the root — covers Chromium helpers that left the
        // process group (and any survivors after the group signal).
        for child_pid in descendants.into_iter().rev() {
            if process_is_alive(child_pid).unwrap_or(false) {
                let _ = signal_kill(&child_pid.to_string());
            }
        }

        if process_is_alive(pid)? {
            let status = signal_kill(&pid.to_string())?;
            if !status.success() && process_is_alive(pid)? {
                return Err(io::Error::other(format!(
                    "failed to kill process {pid}: {status}"
                )));
            }
        }

        Ok(())
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

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::process::{Command, Stdio};
    use std::thread;
    use std::time::Duration;

    #[test]
    fn collect_descendant_pids_finds_child_process() {
        let mut child = Command::new("sleep")
            .arg("30")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn sleep");
        let child_pid = child.id();
        let parent_pid = std::process::id();

        // /proc status can lag a moment after spawn.
        let mut found = Vec::new();
        for _ in 0..20 {
            found = collect_descendant_pids(parent_pid);
            if found.contains(&child_pid) {
                break;
            }
            thread::sleep(Duration::from_millis(25));
        }

        let _ = child.kill();
        let _ = child.wait();

        assert!(
            found.contains(&child_pid),
            "expected child {child_pid} among descendants of {parent_pid}, got {found:?}"
        );
    }

    #[test]
    fn force_kill_process_tree_reaps_child() {
        let mut child = Command::new("sleep")
            .arg("60")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn sleep");
        let child_pid = child.id();

        force_kill_process_tree(child_pid).expect("kill tree");
        // Parent must wait() to reap the zombie; kill -0 still succeeds on zombies.
        let status = child.wait().expect("reap killed child");
        assert!(
            !status.success(),
            "child {child_pid} should have been killed, got {status}"
        );
        assert!(
            !process_is_alive(child_pid).unwrap_or(true),
            "child {child_pid} should be gone after wait()"
        );
    }
}
