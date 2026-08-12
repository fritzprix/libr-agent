//! Keep the main LibrAgent window visible on the Windows taskbar.
//!
//! A leftover `trayIcon` config (without handlers) used to spawn a hidden
//! `tray_icon_app` tool-window. Combined with WebView2 top-level style quirks,
//! minimize could leave the app running with no discoverable taskbar entry.
//!
//! This module re-asserts `WS_EX_APPWINDOW` (and clears `WS_EX_TOOLWINDOW`) and
//! calls Tauri's `set_skip_taskbar(false)` after startup and around minimize.

use log::{debug, warn};
use tauri::{AppHandle, Manager, Runtime};

const MAIN_WINDOW_LABEL: &str = "main";

/// Ensure the main window stays listed on the Windows taskbar.
pub fn ensure_main_window_taskbar_button<R: Runtime>(app: &AppHandle<R>) {
    #[cfg(not(windows))]
    {
        let _ = app;
    }

    #[cfg(windows)]
    {
        let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
            debug!("Taskbar ensure skipped: main window not ready");
            return;
        };

        if let Err(error) = window.set_skip_taskbar(false) {
            warn!("Failed to clear skip_taskbar on main window: {error}");
        }

        if let Some(icon) = app.default_window_icon().cloned() {
            if let Err(error) = window.set_icon(icon) {
                warn!("Failed to re-apply main window icon: {error}");
            }
        }

        match window.hwnd() {
            Ok(hwnd) => apply_appwindow_exstyle(hwnd.0 as isize),
            Err(error) => warn!("Failed to read main window HWND for taskbar fix: {error}"),
        }
    }
}

#[cfg(windows)]
fn apply_appwindow_exstyle(hwnd: isize) {
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, SWP_FRAMECHANGED,
        SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
    };

    if hwnd == 0 {
        return;
    }

    unsafe {
        let hwnd = hwnd as HWND;
        let current = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let updated =
            (current | WS_EX_APPWINDOW as isize) & !(WS_EX_TOOLWINDOW as isize);
        if updated == current {
            return;
        }

        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, updated);
        let _ = SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        );
        debug!(
            "Reasserted WS_EX_APPWINDOW on main window (exstyle 0x{current:X} -> 0x{updated:X})"
        );
    }
}
