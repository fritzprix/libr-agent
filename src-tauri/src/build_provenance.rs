//! Git commit embedded at compile time (see `build.rs`).

pub const BUILD_SHA: &str = env!("LIBRAGENT_BUILD_SHA");

/// `true` when the working tree had uncommitted changes at build time.
pub fn build_dirty() -> bool {
    matches!(env!("LIBRAGENT_BUILD_DIRTY"), "true")
}
