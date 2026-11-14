//! Git commit embedded at compile time (see `build.rs`).

pub const BUILD_SHA: &str = env!("LIBRAGENT_BUILD_SHA");

/// `true` when the working tree had uncommitted changes at build time.
///
/// The flag describes the tracked tree as of the last `build.rs` run (see the
/// `rerun-if-changed` sources in `emit_build_provenance`), not the moment the
/// value is read.
pub fn build_dirty() -> bool {
    matches!(env!("LIBRAGENT_BUILD_DIRTY"), "true")
}
