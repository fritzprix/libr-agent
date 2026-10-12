//! Windows-safe unit tests for spawnSession workspace binding (#2080).
//! (Not behind cfg(not(windows)) — pure path helpers, no Tauri WebView link.)

use std::path::Path;

use tauri_mcp_agent_lib::mcp::builtin::agent::handlers::{
    is_inherit_workspace_override, resolve_spawn_workspace_binding,
};

#[test]
fn inherit_tokens_are_recognized_case_insensitively() {
    for token in ["current", "CURRENT", " . ", "inherit", "Inherit"] {
        assert!(
            is_inherit_workspace_override(token),
            "expected inherit token: {token:?}"
        );
    }
    for token in ["/tmp/ws", "C:\\ws", "relative/path", ""] {
        assert!(
            !is_inherit_workspace_override(token),
            "expected non-inherit token: {token:?}"
        );
    }
}

#[test]
fn resolve_inherit_tokens_use_caller_workspace() {
    let caller = Path::new("/caller/workspace");
    let org = Path::new("/org/workspace");

    for token in ["current", ".", "inherit"] {
        let resolved = resolve_spawn_workspace_binding(Some(token), caller, Some(org))
            .expect("inherit should resolve");
        assert_eq!(resolved, caller);
    }
}

#[test]
fn resolve_explicit_path_wins_over_org_default() {
    let caller = Path::new("/caller/workspace");
    let org = Path::new("/org/workspace");
    let resolved =
        resolve_spawn_workspace_binding(Some("/other/shared"), caller, Some(org)).unwrap();
    assert_eq!(resolved, Path::new("/other/shared"));
}

#[test]
fn resolve_omitted_override_uses_org_root_when_present() {
    let caller = Path::new("/caller/workspace");
    let org = Path::new("/org/workspace");
    let resolved = resolve_spawn_workspace_binding(None, caller, Some(org)).unwrap();
    assert_eq!(resolved, org);
}

#[test]
fn resolve_omitted_override_isolates_plain_session() {
    let caller = Path::new("/caller/workspace");
    assert!(resolve_spawn_workspace_binding(None, caller, None).is_none());
}
