//! Keep the machine from idle-sleeping while LibrAgent is running.
//!
//! - Active for the whole app lifetime when the user preference is on (default).
//! - User preference (`preventSleepDuringAgentWork`, default on) can disable it.
//! - Does not force the display to stay on.
//! - Platform handles are cleared when the preference is off and on process exit.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

/// User setting: prevent sleep while the app is running (default enabled).
static USER_ENABLED: AtomicBool = AtomicBool::new(true);
/// What is currently applied to the OS.
static PLATFORM_ACTIVE: AtomicBool = AtomicBool::new(false);
static SHUTDOWN: AtomicBool = AtomicBool::new(false);

#[cfg(windows)]
static WINDOWS_TX: OnceLock<std::sync::mpsc::Sender<bool>> = OnceLock::new();

#[cfg(any(target_os = "macos", target_os = "linux"))]
struct UnixInhibitState {
    child: Option<std::process::Child>,
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
static UNIX_STATE: OnceLock<std::sync::Mutex<UnixInhibitState>> = OnceLock::new();

/// Apply the user preference from Settings (`preventSleepDuringAgentWork`).
///
/// Defaults to enabled when unset. When enabled, inhibit idle sleep for the
/// whole time the app is running.
pub fn set_user_preference(enabled: bool) {
    USER_ENABLED.store(enabled, Ordering::SeqCst);
    recompute();
}

/// Release any keep-awake hold (call on app exit).
pub fn shutdown() {
    SHUTDOWN.store(true, Ordering::SeqCst);
    recompute();
}

fn recompute() {
    let effective = !SHUTDOWN.load(Ordering::SeqCst) && USER_ENABLED.load(Ordering::SeqCst);

    let previous = PLATFORM_ACTIVE.swap(effective, Ordering::SeqCst);
    if previous == effective {
        return;
    }

    apply(effective);
}

fn apply(needed: bool) {
    #[cfg(windows)]
    {
        apply_windows(needed);
    }

    #[cfg(target_os = "macos")]
    {
        apply_macos(needed);
    }

    #[cfg(target_os = "linux")]
    {
        apply_linux(needed);
    }

    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    {
        let _ = needed;
    }
}

#[cfg(windows)]
fn apply_windows(needed: bool) {
    let tx = WINDOWS_TX.get_or_init(|| {
        let (tx, rx) = std::sync::mpsc::channel::<bool>();
        std::thread::Builder::new()
            .name("libragent-keep-awake".into())
            .spawn(move || {
                use windows_sys::Win32::System::Power::{
                    SetThreadExecutionState, ES_CONTINUOUS, ES_SYSTEM_REQUIRED,
                };

                let mut active = false;
                while let Ok(want) = rx.recv() {
                    if want && !active {
                        // Hold on a long-lived thread: SetThreadExecutionState is
                        // thread-scoped and must not be called from ephemeral workers.
                        let prev =
                            unsafe { SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED) };
                        if prev == 0 {
                            log::warn!("Failed to enable keep-awake (SetThreadExecutionState)");
                        } else {
                            active = true;
                            log::info!("Keep-awake enabled (Windows system required)");
                        }
                    } else if !want && active {
                        let prev = unsafe { SetThreadExecutionState(ES_CONTINUOUS) };
                        if prev == 0 {
                            log::warn!("Failed to clear keep-awake (SetThreadExecutionState)");
                        } else {
                            active = false;
                            log::info!("Keep-awake disabled (Windows)");
                        }
                    }
                }

                if active {
                    unsafe {
                        SetThreadExecutionState(ES_CONTINUOUS);
                    }
                }
            })
            .expect("failed to spawn keep-awake thread");
        tx
    });

    if let Err(error) = tx.send(needed) {
        log::warn!("Failed to update Windows keep-awake state: {error}");
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn unix_state() -> &'static std::sync::Mutex<UnixInhibitState> {
    UNIX_STATE.get_or_init(|| std::sync::Mutex::new(UnixInhibitState { child: None }))
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn stop_unix_child(state: &mut UnixInhibitState) {
    if let Some(mut child) = state.child.take() {
        match child.kill() {
            Ok(()) => {
                let _ = child.wait();
                log::info!("Keep-awake child process stopped");
            }
            Err(error) => {
                log::warn!("Failed to stop keep-awake child process: {error}");
            }
        }
    }
}

#[cfg(target_os = "macos")]
fn apply_macos(needed: bool) {
    let Ok(mut state) = unix_state().lock() else {
        log::warn!("Keep-awake mutex poisoned (macOS)");
        return;
    };

    if needed {
        if state.child.is_some() {
            return;
        }

        // -i: prevent idle sleep. -w <pid>: exit when LibrAgent exits (no orphan).
        let mut command = std::process::Command::new("caffeinate");
        command
            .arg("-i")
            .arg("-w")
            .arg(std::process::id().to_string());
        match command.spawn() {
            Ok(child) => {
                state.child = Some(child);
                log::info!("Keep-awake enabled (macOS caffeinate -i -w)");
            }
            Err(error) => {
                log::warn!("Failed to spawn caffeinate for keep-awake: {error}");
            }
        }
    } else {
        stop_unix_child(&mut state);
    }
}

#[cfg(target_os = "linux")]
fn apply_linux(needed: bool) {
    let Ok(mut state) = unix_state().lock() else {
        log::warn!("Keep-awake mutex poisoned (Linux)");
        return;
    };

    if needed {
        if state.child.is_some() {
            return;
        }

        let mut command = std::process::Command::new("systemd-inhibit");
        command
            .arg("--what=idle:sleep")
            .arg("--who=LibrAgent")
            .arg("--why=LibrAgent is running")
            .arg("--mode=block")
            .arg("sleep")
            .arg("infinity");

        match command.spawn() {
            Ok(child) => {
                state.child = Some(child);
                log::info!("Keep-awake enabled (Linux systemd-inhibit)");
            }
            Err(error) => {
                log::warn!("Keep-awake unavailable on Linux (systemd-inhibit failed): {error}");
            }
        }
    } else {
        stop_unix_child(&mut state);
    }
}
