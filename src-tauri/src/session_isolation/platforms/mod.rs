// Shared Unix logic (private to this module hierarchy, used by linux/macos)
#[cfg(unix)]
pub(crate) mod unix;

// Platform-specific implementations
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
mod windows_python;
#[cfg(target_os = "windows")]
pub use windows::*;

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "linux")]
pub use linux::*;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::*;

// For Unix platforms (Linux/macOS), we also need to expose the basic/medium commands
// since linux/macos modules only implement high isolation.
#[cfg(unix)]
pub use unix::{create_basic_isolated_command, create_medium_isolated_command};

// Fallback for generic Unix systems (not Linux/macOS)
#[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
pub use unix::create_high_isolated_command;
