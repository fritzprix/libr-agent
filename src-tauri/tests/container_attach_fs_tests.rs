//! Attach staging path matching (Windows verbatim vs disk prefixes).

use std::path::{Path, PathBuf};
use tauri_mcp_agent_lib::mcp::builtin::utils::relative_path_under_base;
use tauri_mcp_agent_lib::services::container_attach_fs::AttachSessionInfo;

#[test]
fn relative_path_under_base_handles_exact_match() {
    let base = PathBuf::from(if cfg!(windows) {
        r"C:\staging"
    } else {
        "/tmp/staging"
    });
    let relative = relative_path_under_base(&base, &base).expect("exact match");
    assert!(relative.as_os_str().is_empty() || relative == Path::new("."));
}

#[cfg(windows)]
#[test]
fn attach_container_path_maps_verbatim_host_file() {
    let host_workspace = Path::new(r"C:\Users\test\staging");
    let info = AttachSessionInfo {
        container: "harbor-main",
        workdir: "/workspace",
        host_workspace,
    };
    let verbatim = PathBuf::from(r"\\?\C:\Users\test\staging\analysis.py");
    assert_eq!(
        info.container_path_for_host_file(&verbatim)
            .expect("verbatim under staging"),
        "/workspace/analysis.py"
    );
}

#[cfg(windows)]
#[test]
fn relative_path_matches_case_insensitive_drive() {
    let base = Path::new(r"C:\Users\test\staging");
    let file = Path::new(r"c:\Users\test\staging\analysis.py");
    let relative = relative_path_under_base(file, base).expect("case-insensitive match");
    assert_eq!(relative, PathBuf::from("analysis.py"));
}
