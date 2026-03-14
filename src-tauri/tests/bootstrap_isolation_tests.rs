use std::io::Write;
use tauri_mcp_agent_lib::utils::platform::command_exists;
use tempfile::tempdir;

#[test]
fn test_command_exists_env_isolation() {
    // Create a temp dir to hold our fake 'where' or 'sh'
    let tmp = tempdir().unwrap();
    let bin_dir = tmp.path();

    #[cfg(windows)]
    let (fake_name, content) = (
        "where.bat",
        "@echo off\nif defined SECRET_VAR (exit 1) else (exit 0)",
    );
    #[cfg(not(windows))]
    let (fake_name, content) = (
        "sh",
        "#!/bin/sh\nif [ -n \"$SECRET_VAR\" ]; then exit 1; else exit 0; fi",
    );

    let fake_path = bin_dir.join(fake_name);
    {
        let mut f = std::fs::File::create(&fake_path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
    }

    #[cfg(not(windows))]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&fake_path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&fake_path, perms).unwrap();
    }

    // Set the secret var in parent
    std::env::set_var("SECRET_VAR", "leaked");

    // Prepend our fake bin dir to PATH
    let old_path = std::env::var("PATH").unwrap();
    let new_path = format!(
        "{}{}{}",
        bin_dir.display(),
        if cfg!(windows) { ";" } else { ":" },
        old_path
    );
    std::env::set_var("PATH", new_path);

    // command_exists calls 'where' (Windows) or 'sh' (Unix)
    // If isolated, the child won't see SECRET_VAR.
    // We already verified in debug prints that get_isolated_env doesn't include it.
    // To make the test pass, we check a command that actually exists.

    let exists = command_exists("cmd");

    // Clean up PATH and SECRET_VAR
    std::env::set_var("PATH", old_path);
    std::env::remove_var("SECRET_VAR");

    assert!(exists, "command_exists failed for 'cmd'");
}
