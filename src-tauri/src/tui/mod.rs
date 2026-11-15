//! `libragent --tui` entry point, terminal set-up/restore (#2761), and chat
//! core (#2762).
//!
//! No GUI window is created. The backend is reached through the shared
//! startup path (#2756 — attach to a live backend or host one here). LLM
//! calls keep working through the Rust path / hidden WebView. The chat
//! screen (transcript, composer, streaming poll, tool approval, askUser,
//! secret) lives in [`chat`].
//!
//! # Windows console spike (issue #2761, acceptance criterion 3)
//!
//! The release `libragent` binary is `windows_subsystem = "windows"`; a
//! GUI-subsystem process must keep that or every GUI start pops a console
//! window. Two approaches were measured on Windows 11 (conhost, the machine
//! default; PowerShell 5.1.26100 and PowerShell 7):
//!
//! | measurement                                    | GUI-subsystem + `AttachConsole(ATTACH_PARENT_PROCESS)` | console-subsystem binary |
//! | ---------------------------------------------- | ------------------------------------------------------ | ------------------------ |
//! | console attach / mode get / mode set / write   | all succeed under conhost                              | native, no attach needed |
//! | PowerShell 5.1 / 7 wait for a 3 s process      | **0.0 s — the shell does not wait**                    | **3.0 s — the shell waits** |
//! | `cmd` batch wait + exit code                   | batch waits, exit code propagates                      | batch waits, exit code propagates |
//!
//! So `AttachConsole` can fix the I/O of a GUI-subsystem binary, but the
//! shell-wait rule is decided by the PE subsystem at launch, and
//! `libragent.exe` cannot change it. Chosen approach: a **console-subsystem
//! launcher** — the separate `libragent-tui` binary (same library, same
//! profile, same database). PowerShell and cmd wait for it and see its exit
//! code; its stdout/stderr pipe normally. `libragent --tui` on Windows
//! forwards to the sibling binary so the documented command still works
//! from scripts and batch files; interactive terminals should run
//! `libragent-tui` directly. Windows Terminal was not installed on the
//! measurement machine; the wait behaviour is a shell-side property of the
//! PE subsystem (independent of terminal host), and WT implements the same
//! Win32 console API for I/O that conhost provided above.

mod chat;
mod sessions;

use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen},
};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// Set while this process holds the terminal in raw mode on the alternate
/// screen, so restore is a no-op for the GUI and after the first restore.
static TERMINAL_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Flag that selects TUI mode on any entry binary.
pub const TUI_FLAG: &str = "--tui";

/// Poll interval while waiting for terminal input or a backend record.
pub const TUI_POLL_INTERVAL: Duration = Duration::from_millis(200);

/// True when the given arguments contain [`TUI_FLAG`].
pub fn args_want_tui(args: impl IntoIterator<Item = impl AsRef<str>>) -> bool {
    args.into_iter().any(|arg| arg.as_ref() == TUI_FLAG)
}

/// True when this process was started with [`TUI_FLAG`].
pub fn wants_tui() -> bool {
    args_want_tui(std::env::args())
}

/// Path of the console-subsystem launcher next to this executable.
pub fn console_launcher_path_in(exe_dir: &Path) -> PathBuf {
    let name = if cfg!(windows) {
        "libragent-tui.exe"
    } else {
        "libragent-tui"
    };
    exe_dir.join(name)
}

/// Windows: run the console-subsystem sibling and exit with its code, so
/// scripts and batch files get a waited-on TUI with a real exit code.
///
/// Before spawning, `AttachConsole(ATTACH_PARENT_PROCESS)` joins the
/// launching terminal (measured to succeed when started from a terminal;
/// ignored when there is none — e.g. a double-click — and then the console
/// child gets its own window).
#[cfg(windows)]
pub fn forward_to_console_launcher() -> i32 {
    use windows_sys::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};

    // SAFETY: ATTACH_PARENT_PROCESS is the documented "join my parent's
    // console" argument; failure just means no console, which is handled.
    unsafe { AttachConsole(ATTACH_PARENT_PROCESS) };

    let exe_dir = match std::env::current_exe() {
        Ok(exe) => exe.parent().map(|dir| dir.to_path_buf()),
        Err(_) => None,
    };
    let Some(launcher) = exe_dir
        .map(|dir| console_launcher_path_in(&dir))
        .filter(|path| path.is_file())
    else {
        eprintln!("error: libragent-tui binary not found next to libragent");
        return 1;
    };

    let status = std::process::Command::new(&launcher)
        .args(std::env::args_os().skip(1))
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status();
    match status {
        Ok(status) => status.code().unwrap_or(1),
        Err(err) => {
            eprintln!("error: cannot start {}: {err}", launcher.display());
            1
        }
    }
}

/// The backend state line shown while the chat waits for a live record.
pub fn status_line(
    descriptor: Option<&crate::server::backend_descriptor::BackendDescriptor>,
    own_pid: u32,
) -> String {
    match descriptor {
        None => "Backend: starting (no live record yet)".to_string(),
        Some(d) if d.pid == own_pid => {
            format!("Backend: hosting here (pid {}, port {})", d.pid, d.port)
        }
        Some(d) => format!("Backend: attached to pid {} at port {}", d.pid, d.port),
    }
}

/// Restore the terminal to cooked, non-alternate-screen mode if the TUI set
/// it up. Safe to call from any exit path, any number of times; every error
/// is ignored because there is nothing left to do if the terminal is gone.
pub(crate) fn restore_terminal() {
    if !TERMINAL_ACTIVE.swap(false, Ordering::SeqCst) {
        return;
    }
    let _ = crossterm::terminal::disable_raw_mode();
    let _ = execute!(std::io::stdout(), LeaveAlternateScreen);
}

/// Restore the terminal before any panic report reaches the user.
fn install_terminal_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        previous(info);
    }));
}

/// Enter raw mode and the alternate screen. The terminal state is
/// process-global, so this runs on the main thread before the backend
/// starts: a terminal that cannot present the TUI must stop the process,
/// not leave an invisible backend running.
///
/// Windows only: when stdout is not a TTY (scripted smoke with redirected
/// pipes), skip raw mode / alternate screen so the hidden WebView can still
/// boot — the chat front has nowhere to draw anyway, and taking the console
/// is what breaks WebView2 page JS (#2784). Linux/macOS keep the original
/// error for non-tty launches: scripts there attach a real pty
/// (`spawn_tui` in `scripts/tui_hidden_webview_smoke.py`), so the
/// "cannot enter raw mode" failure stays the coverage for that case.
fn setup_terminal() -> Result<(), String> {
    install_terminal_panic_hook();
    #[cfg(windows)]
    {
        use std::io::IsTerminal;
        if !std::io::stdout().is_terminal() {
            // Keep TERMINAL_ACTIVE false so restore stays a no-op.
            return Ok(());
        }
    }
    TERMINAL_ACTIVE.store(true, Ordering::SeqCst);
    if crossterm::terminal::enable_raw_mode().is_err() {
        return Err(
            "this terminal cannot enter raw mode; run libragent-tui from a real terminal"
                .to_string(),
        );
    }
    let stdout = std::io::stdout();
    let mut stdout = stdout.lock();
    if execute!(stdout, EnterAlternateScreen).is_err() {
        return Err(
            "cannot enter the alternate screen; run libragent-tui from a real terminal".to_string(),
        );
    }
    Ok(())
}

/// Deferred Windows terminal setup (#2784) can only fail after the backend
/// has already started. A hidden WebView with no usable terminal is the
/// exact state `setup_terminal` promises never to leave running, so restore
/// the terminal and take the whole process down with a non-zero code.
/// Documented smoke case: a Windows TTY launch whose console cannot enter
/// raw mode reaches this after the deferred readiness wait and exits 1.
fn abort_without_terminal(err: &str) -> ! {
    restore_terminal();
    eprintln!("error: {err}");
    std::process::exit(1);
}

/// The TUI front: chat core on an already-prepared terminal (#2762).
/// Runs on a background thread; the main thread owns the Tauri event loop
/// (the WebView requires the main thread on Windows).
fn front_of_house() {
    let mut stdout = std::io::stdout();
    chat::run_chat_front(&mut stdout);
}

/// Run `libragent --tui` in this process: bootstrap like every entry point,
/// present the terminal, and run the backend with its window hidden.
///
/// The hidden WebView still answers LLM calls — the phase-1 allowance of
/// #2761 — because this is the same Tauri app the GUI runs, only with
/// `visible = false`.
pub fn run_tui() -> i32 {
    // Bootstrap starts no backend and may exit(1) on its own, so it runs
    // on the normal screen where its messages stay readable.
    crate::bootstrap::load_dotenv_files();
    let db_url = crate::bootstrap::prepare_profile_runtime();

    // On Windows, taking the console (raw mode + alternate screen) *before*
    // Tauri starts prevents WebView2 from running page JS, so
    // `frontend-ready` never arrives and LLM turns hang (#2784). When stdout
    // is a real TTY, defer terminal setup until the hidden WebView has
    // announced readiness. When stdout is redirected (scripted smoke),
    // setup_terminal's Windows-only skip leaves raw mode off and the front
    // starts immediately — the same path that keeps WebView2 healthy under
    // pipes.
    // On other platforms, fail early if the terminal cannot present a TUI
    // instead of leaving an invisible backend.
    #[cfg(windows)]
    {
        use std::io::IsTerminal;
        if std::io::stdout().is_terminal() {
            install_terminal_panic_hook();
            std::thread::spawn(|| {
                let ready = crate::lifecycle::frontend_ready::wait_until_ready_blocking(
                    std::time::Duration::from_secs(90),
                );
                if !ready {
                    log::warn!(
                        "frontend did not signal readiness within 90s; starting TUI front anyway"
                    );
                }
                if let Err(err) = setup_terminal() {
                    abort_without_terminal(&err);
                }
                front_of_house();
            });
        } else if let Err(err) = setup_terminal() {
            restore_terminal();
            eprintln!("error: {err}");
            return 1;
        } else {
            std::thread::spawn(front_of_house);
        }
    }
    #[cfg(not(windows))]
    {
        if let Err(err) = setup_terminal() {
            restore_terminal();
            eprintln!("error: {err}");
            return 1;
        }
        std::thread::spawn(front_of_house);
    }

    // Tauri's App::run ends the process with std::process::exit; the
    // terminal is restored by the RunEvent::Exit handler, so nothing after
    // this call runs.
    crate::lifecycle::run_with_sqlite_sync_and_visibility(db_url, false);
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::backend_descriptor::BackendDescriptor;

    fn descriptor(pid: u32, port: u16) -> BackendDescriptor {
        BackendDescriptor {
            pid,
            port,
            started_at_ms: 1,
        }
    }

    #[cfg(windows)]
    #[test]
    fn setup_terminal_skips_raw_mode_when_stdout_is_not_a_tty() {
        // `cargo test` runs with piped stdout — exactly the scripted-smoke
        // case: the Windows skip must report success and leave
        // TERMINAL_ACTIVE false so restore_terminal stays a no-op.
        assert!(setup_terminal().is_ok());
        assert!(!TERMINAL_ACTIVE.load(Ordering::SeqCst));
    }

    #[test]
    fn tui_flag_detection() {
        let with_flag = vec!["libragent".to_string(), "--tui".to_string()];
        let without = vec!["libragent".to_string(), "--demo".to_string()];
        assert!(args_want_tui(with_flag.iter()));
        assert!(!args_want_tui(without.iter()));
        assert!(!args_want_tui(Vec::<String>::new().iter()));
        assert!(args_want_tui(["libragent", "--profile", "dev", "--tui"]));
    }

    #[test]
    fn launcher_path_uses_sibling_name_for_platform() {
        let dir = Path::new(if cfg!(windows) { "C:\\app" } else { "/opt/app" });
        let path = console_launcher_path_in(dir);
        let expected = if cfg!(windows) {
            "libragent-tui.exe"
        } else {
            "libragent-tui"
        };
        assert_eq!(path.file_name().unwrap().to_string_lossy(), expected);
        assert_eq!(path.parent(), Some(dir));
    }

    #[test]
    fn status_line_distinguishes_hosting_from_attaching() {
        assert_eq!(
            status_line(None, 100),
            "Backend: starting (no live record yet)"
        );
        assert_eq!(
            status_line(Some(&descriptor(100, 8420)), 100),
            "Backend: hosting here (pid 100, port 8420)"
        );
        assert_eq!(
            status_line(Some(&descriptor(200, 8420)), 100),
            "Backend: attached to pid 200 at port 8420"
        );
    }
}
