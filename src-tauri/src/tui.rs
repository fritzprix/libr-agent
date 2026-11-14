//! `libragent --tui` entry point and terminal set-up/restore (#2761).
//!
//! Phase 1 scope: no GUI window is created, the backend is reached through
//! the shared startup path (#2756 — attach to a live backend or host one
//! here), and LLM calls keep working through the hidden WebView (the phase-1
//! allowance, the same mechanism `liblet` uses). The chat UI itself is
//! #2762; this module presents the terminal, shows backend state, and quits
//! cleanly on `q` / Ctrl+C.
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

use crossterm::{
    cursor::MoveTo,
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Flag that selects TUI mode on any entry binary.
pub const TUI_FLAG: &str = "--tui";

/// Poll interval while waiting for terminal input or a backend record.
pub const TUI_POLL_INTERVAL: Duration = Duration::from_millis(200);

/// True when the given arguments contain [`TUI_FLAG`].
pub fn args_want_tui<'a>(args: impl IntoIterator<Item = &'a String>) -> bool {
    args.into_iter().any(|arg| arg == TUI_FLAG)
}

/// True when this process was started with [`TUI_FLAG`].
pub fn wants_tui() -> bool {
    std::env::args().any(|arg| arg == TUI_FLAG)
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

/// The backend state line shown in the TUI, given the live descriptor (if
/// any) and this process's pid. Pure so it is testable without a terminal.
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

/// Lines the TUI prints above the prompt. Pure for the same reason.
pub fn status_screen(
    descriptor: Option<&crate::server::backend_descriptor::BackendDescriptor>,
    own_pid: u32,
) -> Vec<String> {
    vec![
        " LibrAgent TUI (phase 1)".to_string(),
        String::new(),
        status_line(descriptor, own_pid),
        String::new(),
        " q or Ctrl+C quits. The chat screen lands with #2762.".to_string(),
    ]
}

/// Restore the terminal to cooked, non-alternate-screen mode. Idempotent;
/// every error is ignored because there is nothing left to do if the
/// terminal is already gone.
fn restore_terminal() {
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
fn setup_terminal() -> Result<(), String> {
    install_terminal_panic_hook();
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

/// The TUI front: backend status and quit keys on an already-prepared
/// terminal. Runs on a background thread; the main thread owns the Tauri
/// event loop (the WebView requires the main thread on Windows).
///
/// stdout is locked per render frame only, never across the event poll:
/// the debug-only MCP bridge plugin logs with `println!` from the main
/// thread during plugin init, and holding the stdout lock here would
/// deadlock that `println!` (the backend would never boot).
fn front_of_house() {
    let stdout = std::io::stdout();
    let own_pid = std::process::id();
    let mut quit_requested = false;
    loop {
        let descriptor = crate::server::backend_descriptor::find_live_backend();
        let mut lines = status_screen(descriptor.as_ref(), own_pid);
        if quit_requested {
            lines.push(" Quitting (waiting for the backend to shut down)...".to_string());
        }

        {
            let mut stdout = stdout.lock();
            execute!(stdout, MoveTo(0, 0), Clear(ClearType::FromCursorDown)).ok();
            for line in &lines {
                writeln!(stdout, "{line}").ok();
            }
            stdout.flush().ok();
        }

        if event::poll(TUI_POLL_INTERVAL).unwrap_or(false) {
            if let Ok(Event::Key(key)) = event::read() {
                // Windows and Linux both deliver repeat/press events only
                // in raw mode; ignore releases for cleanliness.
                if key.kind == KeyEventKind::Press || key.kind == KeyEventKind::Repeat {
                    let ctrl_c = key.modifiers.contains(KeyModifiers::CONTROL)
                        && matches!(key.code, KeyCode::Char('c'));
                    if ctrl_c || matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q')) {
                        quit_requested = true;
                    }
                }
            }
        }

        if quit_requested {
            // Graceful exit only: the RunEvent::Exit handler unpublishes the
            // backend record. Until the app handle exists (still starting),
            // keep looping so a very early q is honoured, not dropped.
            if let Some(handle) = crate::state::get_app_handle() {
                restore_terminal();
                // Returning is enough: the main thread stays blocked in the
                // Tauri event loop until this exit request tears it down.
                handle.exit(0);
                return;
            }
        }
    }
}

/// Run `libragent --tui` in this process: bootstrap like every entry point,
/// present the terminal, and run the backend with its window hidden.
///
/// The hidden WebView still answers LLM calls — the phase-1 allowance of
/// #2761 — because this is the same Tauri app the GUI runs, only with
/// `visible = false`.
pub fn run_tui() -> i32 {
    // Set the terminal before booting anything: if this process cannot
    // present a TUI, fail now instead of leaving an invisible backend.
    if let Err(err) = setup_terminal() {
        restore_terminal();
        eprintln!("error: {err}");
        return 1;
    }

    crate::bootstrap::load_dotenv_files();
    let db_url = crate::bootstrap::prepare_profile_runtime();

    let front = std::thread::spawn(front_of_house);

    // Blocks until the event loop exits (quit keys call AppHandle::exit).
    crate::lifecycle::run_with_sqlite_sync_and_visibility(db_url, false);

    restore_terminal();
    // If the event loop ended without the front thread quitting (e.g. a
    // tray quit), let it finish its last render and exit.
    let _ = front.join();
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

    #[test]
    fn tui_flag_detection() {
        let with_flag = vec!["libragent".to_string(), "--tui".to_string()];
        let without = vec!["libragent".to_string(), "--demo".to_string()];
        assert!(args_want_tui(with_flag.iter()));
        assert!(!args_want_tui(without.iter()));
        assert!(!args_want_tui(Vec::<String>::new().iter()));
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

    #[test]
    fn status_screen_keeps_the_quit_hint() {
        let lines = status_screen(None, 100);
        assert!(lines[0].contains("LibrAgent TUI"));
        assert!(lines[2].contains("starting"));
        assert!(lines.last().unwrap().contains("Ctrl+C"));
    }
}
