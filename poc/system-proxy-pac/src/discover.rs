#[cfg(windows)]
mod imp {
    pub use super::super::windows_sysproxy::{discover as discover_impl, resolve as resolve_impl};
}

#[cfg(target_os = "macos")]
mod imp {
    pub use super::super::macos_sysproxy::{discover as discover_impl, resolve as resolve_impl};
}

#[cfg(not(any(windows, target_os = "macos")))]
mod imp {
    pub use super::super::fallback_env::{discover as discover_impl, resolve as resolve_impl};
}

pub use imp::{discover_impl as discover, resolve_impl as resolve};