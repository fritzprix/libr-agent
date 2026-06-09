//! Shared conda/nvm discovery for PATH probing and persistent-shell bootstrap.
//!
//! Sources only known integration scripts — never full shell rc files.

use std::path::{Path, PathBuf};

const CONDA_ROOT_DIR_NAMES: &[&str] = &["miniconda3", "anaconda3", "mambaforge", "miniforge3"];

pub fn home_dir() -> Option<PathBuf> {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()
        .map(PathBuf::from)
}

/// Discover `etc/profile.d/conda.sh` on Unix-like systems.
#[cfg(unix)]
pub fn discover_conda_sh() -> Option<PathBuf> {
    if let Ok(conda_exe) = std::env::var("CONDA_EXE") {
        if let Some(path) = conda_sh_from_conda_exe(Path::new(&conda_exe)) {
            return Some(path);
        }
    }

    if let Some(home) = home_dir() {
        for dir_name in CONDA_ROOT_DIR_NAMES {
            let candidate = home.join(dir_name).join("etc/profile.d/conda.sh");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    None
}

#[cfg(unix)]
pub fn resolve_conda_sh_path(conda_exe: &Path) -> Option<PathBuf> {
    let bin_dir = conda_exe.parent()?;
    let root = bin_dir.parent()?;
    Some(root.join("etc/profile.d/conda.sh"))
}

#[cfg(unix)]
fn conda_sh_from_conda_exe(conda_exe: &Path) -> Option<PathBuf> {
    resolve_conda_sh_path(conda_exe).filter(|candidate| candidate.is_file())
}

/// Discover `nvm.sh` on Unix-like systems.
#[cfg(unix)]
pub fn discover_nvm_sh() -> Option<PathBuf> {
    if let Ok(nvm_dir) = std::env::var("NVM_DIR") {
        let candidate = PathBuf::from(nvm_dir).join("nvm.sh");
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    home_dir()
        .map(|home| home.join(".nvm").join("nvm.sh"))
        .filter(|path| path.is_file())
}

/// Build shell lines that source conda/nvm integration scripts when present.
#[cfg(unix)]
pub fn build_unix_integration_source_script() -> Option<String> {
    let mut parts = Vec::new();

    for path in [discover_conda_sh(), discover_nvm_sh()]
        .into_iter()
        .flatten()
    {
        let quoted = shell_single_quote(&path)?;
        parts.push(format!("if [ -f {quoted} ]; then . {quoted}; fi"));
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n"))
    }
}

#[cfg(windows)]
pub fn discover_conda_path_prefixes() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Ok(conda_exe) = std::env::var("CONDA_EXE") {
        if let Some(root) = conda_root_from_exe(Path::new(&conda_exe)) {
            push_unique_path(&mut paths, root.join("condabin"));
            push_unique_path(&mut paths, root.join("Scripts"));
        }
    }

    if let Some(home) = home_dir() {
        for dir_name in CONDA_ROOT_DIR_NAMES {
            let root = home.join(dir_name);
            push_unique_path(&mut paths, root.join("condabin"));
            push_unique_path(&mut paths, root.join("Scripts"));
        }
    }

    paths
}

#[cfg(windows)]
pub fn discover_nvm_home() -> Option<PathBuf> {
    if let Ok(nvm_home) = std::env::var("NVM_HOME") {
        let path = PathBuf::from(nvm_home);
        if path.is_dir() {
            return Some(path);
        }
    }

    home_dir()
        .map(|home| home.join("AppData").join("Roaming").join("nvm"))
        .filter(|path| path.is_dir())
}

pub fn shell_single_quote(path: &Path) -> Option<String> {
    let path = path.to_str()?;
    Some(format!("'{}'", path.replace('\'', "'\\''")))
}

#[cfg(windows)]
pub fn powershell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

#[cfg(windows)]
fn conda_root_from_exe(conda_exe: &Path) -> Option<PathBuf> {
    let scripts_or_bin = conda_exe.parent()?;
    let root = scripts_or_bin.parent()?;
    root.exists().then(|| root.to_path_buf())
}

#[cfg(windows)]
fn push_unique_path(paths: &mut Vec<PathBuf>, candidate: PathBuf) {
    if candidate.is_dir() && !paths.iter().any(|existing| existing == &candidate) {
        paths.push(candidate);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_conda_sh_path_maps_exe_to_profile_script() {
        let conda_exe = PathBuf::from("/opt/miniconda3/bin/conda");
        let sh = resolve_conda_sh_path(&conda_exe).expect("path should resolve");
        assert_eq!(sh, PathBuf::from("/opt/miniconda3/etc/profile.d/conda.sh"));
    }

    #[test]
    fn shell_single_quote_escapes_single_quotes() {
        let quoted = shell_single_quote(Path::new("/tmp/a'b/c")).expect("utf-8 path");
        assert_eq!(quoted, "'/tmp/a'\\''b/c'");
    }

    #[test]
    fn shell_single_quote_rejects_non_utf8_paths() {
        use std::ffi::OsStr;

        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            let non_utf8 = OsStr::from_bytes(b"/tmp/\xFF");
            assert!(shell_single_quote(Path::new(non_utf8)).is_none());
        }

        #[cfg(not(unix))]
        {
            let _ = OsStr::new("unused");
        }
    }
}
