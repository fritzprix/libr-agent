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
pub(crate) fn process_is_alive(pid: u32) -> io::Result<bool> {
    Ok(Command::new("kill")
        .args(["-0", &pid.to_string()])
        .status()?
        .success())
}

#[cfg(windows)]
pub(crate) fn process_is_alive(pid: u32) -> io::Result<bool> {
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

/// True when at least one live process belongs to process group `pgid`.
///
/// Used when the group leader is already dead: surviving members still carry
/// `PGID == leader pid` (issue #2652), so the group id stays a safe,
/// ownership-scoped signal target even though the leader is gone.
#[cfg(unix)]
fn unix_process_group_has_members(pgid: u32) -> bool {
    let Ok(output) = Command::new("ps").args(["-A", "-o", "pgid="]).output() else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .filter_map(|value| value.parse::<u32>().ok())
        .any(|value| value == pgid)
}

/// Collect child processes keyed by parent process ID.
///
/// On Linux, this inspects `/proc/*/status` for zero-process-spawn performance.
#[cfg(target_os = "linux")]
fn build_children_by_ppid() -> std::collections::HashMap<u32, Vec<u32>> {
    use std::collections::HashMap;
    use std::fs;

    let mut children_by_ppid: HashMap<u32, Vec<u32>> = HashMap::new();
    let Ok(entries) = fs::read_dir("/proc") else {
        return children_by_ppid;
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
    children_by_ppid
}

/// Parse the output of `ps -A -o pid=,ppid=` into a map of parent PID to child PIDs.
///
/// Ignores blank lines, lines with non-numeric entries, and processes with PID <= 1 (launchd / init).
#[cfg(unix)]
#[cfg_attr(target_os = "linux", allow(dead_code))]
fn parse_ps_pid_ppid(stdout: &str) -> std::collections::HashMap<u32, Vec<u32>> {
    use std::collections::HashMap;

    let mut children_by_ppid: HashMap<u32, Vec<u32>> = HashMap::new();
    for line in stdout.lines() {
        let mut parts = line.split_whitespace();
        let (Some(pid_str), Some(ppid_str)) = (parts.next(), parts.next()) else {
            continue;
        };
        if let (Ok(pid), Ok(ppid)) = (pid_str.parse::<u32>(), ppid_str.parse::<u32>()) {
            if pid > 1 {
                children_by_ppid.entry(ppid).or_default().push(pid);
            }
        }
    }
    children_by_ppid
}

/// Collect child processes keyed by parent process ID.
///
/// On macOS and other non-Linux Unix platforms lacking `/proc`, this queries `ps -A -o pid=,ppid=`.
#[cfg(all(unix, not(target_os = "linux")))]
fn build_children_by_ppid() -> std::collections::HashMap<u32, Vec<u32>> {
    let Ok(output) = Command::new("ps").args(["-A", "-o", "pid=,ppid="]).output() else {
        return std::collections::HashMap::new();
    };
    if !output.status.success() {
        return std::collections::HashMap::new();
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_ps_pid_ppid(&stdout)
}

/// Collect every descendant of `root` by walking parent-child process relationships.
///
/// Chromium sandbox helpers often call `setpgid`/`setsid`, so killing only the
/// sidecar process group leaves orphaned browser processes that keep holding
/// the automation session open. PPID ancestry still reaches those children.
#[cfg(unix)]
fn collect_descendant_pids(root: u32) -> Vec<u32> {
    use std::collections::{HashSet, VecDeque};

    let children_by_ppid = build_children_by_ppid();
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

/// Send SIGKILL to a single PID (`pid_or_group > 0`) or a process group
/// (`pid_or_group < 0`) via `kill(2)`.
///
/// The syscall is used instead of spawning `/usr/bin/kill`: some `kill`
/// binaries accept a negative id, exit 0, and silently deliver nothing to the
/// group members (issue #2652). `kill(2)` is the kernel contract we rely on.
#[cfg(unix)]
fn signal_kill(pid_or_group: i32) -> io::Result<()> {
    // Safety: kill(2) with a plain signal number is safe for any pid value.
    if unsafe { libc::kill(pid_or_group, libc::SIGKILL) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
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
        #[cfg(unix)]
        {
            // Issue #2652: when the root died before this call — a concurrent
            // killer reaped the wrapper first, or it was killed externally —
            // its descendants are invisible to the PPID walk: they have been
            // reparented elsewhere but still share the dead leader's PGID.
            // Sweep that group so a late terminate still removes owned children.
            if unix_process_group_has_members(pid) {
                let _ = signal_kill(-(pid as i32));
            }
        }
        return Ok(());
    }

    #[cfg(unix)]
    {
        // Snapshot descendants before signalling so we still know what to reap
        // after the root/group disappears.
        let descendants = collect_descendant_pids(pid);

        if let Some(group_id) = process_group_id(pid) {
            if group_id == pid {
                let _ = signal_kill(-(pid as i32));
            }
        }

        // Children first, then the root — covers Chromium helpers that left the
        // process group (and any survivors after the group signal).
        for child_pid in descendants.into_iter().rev() {
            if process_is_alive(child_pid).unwrap_or(false) {
                let _ = signal_kill(child_pid as i32);
            }
        }

        if process_is_alive(pid)? {
            if let Err(err) = signal_kill(pid as i32) {
                // ESRCH just means the target exited concurrently with us.
                if err.raw_os_error() != Some(libc::ESRCH) && process_is_alive(pid)? {
                    return Err(io::Error::other(format!(
                        "failed to kill process {pid}: {err}"
                    )));
                }
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
    #[cfg(any(target_os = "linux", target_os = "macos"))]
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

    /// Issue #2652: when the wrapper (group leader) dies before the tree kill
    /// arrives — the race the reported build hit — the long-lived child stays
    /// alive holding the dead leader's PGID. The sweep must still reach it.
    #[cfg(unix)]
    #[test]
    fn force_kill_process_tree_sweeps_group_when_root_already_dead() {
        use std::os::unix::process::CommandExt;

        let pid_file = std::env::temp_dir().join(format!(
            "issue2652-sweep-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));

        let mut command = Command::new("bash");
        command.args([
            "-c",
            &format!("sleep 60 & echo $! > '{}'; wait", pid_file.display()),
        ]);
        command.process_group(0); // group leader, like the isolation manager
        command.stdin(Stdio::null());
        command.stdout(Stdio::null());
        command.stderr(Stdio::null());
        let mut wrapper = command.spawn().expect("spawn bash wrapper");
        let wrapper_pid = wrapper.id();

        let child_pid = {
            let deadline = std::time::Instant::now() + Duration::from_secs(15);
            loop {
                if let Some(pid) = std::fs::read_to_string(&pid_file)
                    .ok()
                    .and_then(|content| content.trim().parse::<u32>().ok())
                {
                    break pid;
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "child sleep never reported its pid"
                );
                thread::sleep(Duration::from_millis(50));
            }
        };
        let _ = std::fs::remove_file(&pid_file);

        // Simulate the race: the wrapper dies first, leaving the child alive
        // with PGID == wrapper pid and reparented away from the tree walk.
        force_kill_process(wrapper_pid).expect("kill wrapper");
        wrapper.wait().expect("reap wrapper"); // zombies still answer kill -0

        force_kill_process_tree(wrapper_pid).expect("sweep dead leader's group");

        // The child is reparented to init, so once dead it is reaped there and
        // kill -0 starts failing.
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        while process_is_alive(child_pid).unwrap_or(false) && std::time::Instant::now() < deadline {
            thread::sleep(Duration::from_millis(100));
        }
        assert!(
            !process_is_alive(child_pid).unwrap_or(true),
            "child {child_pid} must be swept with its dead leader's group"
        );
    }

    #[test]
    fn parse_ps_pid_ppid_handles_spaces_blanks_and_excludes_init() {
        let sample = "
           1       0
           2       0
         100       1
         200     100
         201     100
         invalid line
         999
        ";
        let map = parse_ps_pid_ppid(sample);
        // PID 1 should be excluded
        assert!(!map.values().any(|children| children.contains(&1)));
        // PID 2 has ppid 0
        assert_eq!(map.get(&0), Some(&vec![2]));
        // PID 100 has ppid 1
        assert_eq!(map.get(&1), Some(&vec![100]));
        // PID 200, 201 have ppid 100
        assert_eq!(map.get(&100), Some(&vec![200, 201]));
        // Invalid lines ignored
        assert_eq!(map.get(&999), None);
    }
}
