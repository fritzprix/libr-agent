use std::env;
use std::io;
use std::path::{Path, PathBuf};

#[path = "build_support/bundled_skills.rs"]
mod bundled_skills;
#[path = "build_support/session_relay.rs"]
mod session_relay;

fn profile_output_dir(out_dir: &Path) -> Option<PathBuf> {
    out_dir
        .ancestors()
        .find(|ancestor| ancestor.file_name().and_then(|name| name.to_str()) == Some("build"))
        .and_then(Path::parent)
        .map(Path::to_path_buf)
}

fn sync_bundled_skills_into_profile_output() -> io::Result<()> {
    let manifest_dir = PathBuf::from(
        env::var("CARGO_MANIFEST_DIR").map_err(|error| io::Error::other(error.to_string()))?,
    );
    let source_dir = manifest_dir.join("bundled_skills");
    if !source_dir.exists() {
        return Ok(());
    }

    let out_dir =
        PathBuf::from(env::var("OUT_DIR").map_err(|error| io::Error::other(error.to_string()))?);
    let Some(profile_dir) = profile_output_dir(&out_dir) else {
        return Err(io::Error::other(format!(
            "Failed to resolve target profile directory from OUT_DIR={}",
            out_dir.display()
        )));
    };

    let deployed_dir = profile_dir.join("bundled_skills");
    bundled_skills::mirror_bundled_skills(&source_dir, &deployed_dir)
}

fn sync_bundled_assistants_into_profile_output() -> io::Result<()> {
    let manifest_dir = PathBuf::from(
        env::var("CARGO_MANIFEST_DIR").map_err(|error| io::Error::other(error.to_string()))?,
    );
    let source_dir = manifest_dir.join("bundled_assistants");
    if !source_dir.exists() {
        return Ok(());
    }

    let out_dir =
        PathBuf::from(env::var("OUT_DIR").map_err(|error| io::Error::other(error.to_string()))?);
    let Some(profile_dir) = profile_output_dir(&out_dir) else {
        return Err(io::Error::other(format!(
            "Failed to resolve target profile directory from OUT_DIR={}",
            out_dir.display()
        )));
    };

    let deployed_dir = profile_dir.join("bundled_assistants");
    bundled_skills::mirror_bundled_assistants(&source_dir, &deployed_dir)
}

fn windows_msvc_target() -> bool {
    env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
}

/// Bind Common-Controls v6 for every MSVC-linked executable, including the
/// `cargo test --lib` harness.
///
/// tauri-build embeds the same dependency into the app binary via winres, but
/// the lib unit-test harness is not a `bin`/`tests/*.rs` target, so it gets no
/// RT_MANIFEST. Without v6, the loader binds System32 comctl32 5.82 and dies
/// on `TaskDialogIndirect` with `STATUS_ENTRYPOINT_NOT_FOUND` (0xC0000139).
///
/// `cargo:rustc-link-arg-tests` only covers integration-test binaries; the
/// catch-all `rustc-link-arg` is what reaches the lib harness. The linker
/// merges this with tauri-build's existing app manifest (same dependency).
/// See #2122 / tauri-apps/tauri#13419.
fn embed_windows_comctl_v6_manifest_dependency() {
    if !windows_msvc_target() {
        return;
    }
    println!(
        "cargo:rustc-link-arg=/MANIFESTDEPENDENCY:type='win32' \
         name='Microsoft.Windows.Common-Controls' version='6.0.0.0' \
         processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'"
    );
}

/// The `.deb` ships the bundled ONNX Runtime under `/usr/lib/libragent/` so it
/// never overlaps the distro `libonnxruntime*` packages (#2210). Point the app
/// binary there relative to `/usr/bin`. Only for the dynamic-ORT Linux build
/// (`tauri.appimage-ort.conf.json`); linuxdeploy rewrites the AppImage rpath.
fn add_private_onnxruntime_rpath() {
    println!("cargo:rerun-if-env-changed=ORT_PREFER_DYNAMIC_LINK");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("linux")
        || env::var("ORT_PREFER_DYNAMIC_LINK").as_deref() != Ok("1")
    {
        return;
    }
    println!("cargo:rustc-link-arg-bins=-Wl,-rpath,$ORIGIN/../lib/libragent");
}

/// Embeds the session-relay binaries from `LIBRAGENT_RELAY_DIR` (see `build_support/session_relay.rs`).
fn embed_session_relay() -> io::Result<()> {
    println!("cargo:rerun-if-env-changed=LIBRAGENT_RELAY_DIR");
    let out_dir =
        PathBuf::from(env::var("OUT_DIR").map_err(|error| io::Error::other(error.to_string()))?);
    let relay_dir = env::var_os("LIBRAGENT_RELAY_DIR").map(PathBuf::from);
    session_relay::write_embed(&out_dir, relay_dir.as_deref())
}

fn main() {
    println!("cargo:rerun-if-changed=bundled_skills");
    println!("cargo:rerun-if-changed=bundled_assistants");

    if let Err(error) = sync_bundled_skills_into_profile_output() {
        panic!(
            "failed to mirror bundled_skills into the target profile output directory: {}",
            error
        );
    }

    if let Err(error) = sync_bundled_assistants_into_profile_output() {
        panic!(
            "failed to mirror bundled_assistants into the target profile output directory: {}",
            error
        );
    }

    if let Err(error) = embed_session_relay() {
        panic!("failed to embed the session relay: {}", error);
    }

    embed_windows_comctl_v6_manifest_dependency();
    add_private_onnxruntime_rpath();
    tauri_build::build()
}
