use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

fn copy_dir_recursive(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir_all(dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
}

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
    if deployed_dir.exists() {
        fs::remove_dir_all(&deployed_dir)?;
    }
    copy_dir_recursive(&source_dir, &deployed_dir)
}

fn main() {
    println!("cargo:rerun-if-changed=bundled_skills");

    if let Err(error) = sync_bundled_skills_into_profile_output() {
        panic!(
            "failed to mirror bundled_skills into the target profile output directory: {}",
            error
        );
    }

    tauri_build::build()
}
