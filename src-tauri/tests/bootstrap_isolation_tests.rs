use std::io::Write;
use tauri_mcp_agent_lib::utils::platform::command_exists;
use tempfile::tempdir;

#[test]
fn test_command_exists_env_isolation() {
    // Create a temp dir to hold our fake script
    let tmp = tempdir().unwrap();
    let bin_path = tmp.path().to_owned();

    // Use a unique name that won't conflict with 'sh' or 'where'
    #[cfg(windows)]
    let script_name = "check_secret.exe";
    #[cfg(not(windows))]
    let script_name = "check_secret";

    let script_path = bin_path.join(script_name);

    // Create a script that checks for SECRET_VAR and fails if it's present
    let script_content = if cfg!(windows) {
        "@echo off\nif defined SECRET_VAR (exit /b 1) else (exit /b 0)"
    } else {
        "#!/bin/sh\nif [ -n \"$SECRET_VAR\" ]; then exit 1; else exit 0; fi"
    };

    let mut file = std::fs::File::create(&script_path).unwrap();
    file.write_all(script_content.as_bytes()).unwrap();

    #[cfg(not(windows))]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&script_path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script_path, perms).unwrap();
    }

    // Set SECRET_VAR in parent process
    std::env::set_var("SECRET_VAR", "parent-secret");

    // Prepend temp dir to PATH
    let old_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!(
        "{}{}{}",
        bin_path.to_str().unwrap(),
        if cfg!(windows) { ";" } else { ":" },
        old_path
    );
    std::env::set_var("PATH", new_path);

    // 1. Verify our fake script works and detects the secret in current env
    #[cfg(windows)]
    let script_exists_in_current = command_exists("check_secret.exe");
    #[cfg(not(windows))]
    let script_exists_in_current = command_exists("check_secret");

    // Note: command_exists uses get_isolated_env() which clears SECRET_VAR.
    // So even our fake script called via command_exists should return SUCCESS (true).
    assert!(
        script_exists_in_current,
        "Our fake script should be found and return success because SECRET_VAR is stripped"
    );

    // 2. Verify a standard system command still works
    #[cfg(windows)]
    let system_cmd_exists = command_exists("cmd");
    #[cfg(not(windows))]
    let system_cmd_exists = command_exists("ls");

    // Clean up PATH and SECRET_VAR
    std::env::set_var("PATH", old_path);
    std::env::remove_var("SECRET_VAR");

    assert!(
        system_cmd_exists,
        "Standard system command should be found in isolated env"
    );
}
