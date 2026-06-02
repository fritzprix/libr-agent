use std::process::Command;
use tauri_mcp_agent_lib::utils::env::apply_isolated_env;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[test]
fn test_env_isolation_prevents_leakage() {
    // 1. Set a secret environment variable in the host process
    // We use a unique name to avoid conflicts
    let secret_var = "SECRET_LEAK_TEST_UUID_1234";
    unsafe { std::env::set_var(secret_var, "leaked_value") };

    let test_id = uuid::Uuid::new_v4().to_string();
    let output_file = std::env::temp_dir().join(format!("env_leak_test_{}.txt", test_id));
    if output_file.exists() {
        let _ = std::fs::remove_file(&output_file);
    }

    // We construct a python script that writes the env vars to the file
    // We need to properly escape backslashes in paths for python string
    let output_path_str = output_file.to_string_lossy().replace("\\", "/");

    // On Windows, both `python3` (MS Store stub) and `python` (may resolve to App
    // Execution Alias in WindowsApps) fail in piped/no-window subprocess contexts.
    // We resolve the full absolute path to the real python.exe so that:
    //   1. prepare_command sees ".exe" and skips the cmd.exe wrapper
    //   2. The App Execution Alias in WindowsApps is bypassed entirely
    #[cfg(windows)]
    let python_cmd: String = {
        let output = std::process::Command::new("where")
            .arg("python")
            .output()
            .expect("'where' command failed");
        let paths = String::from_utf8(output.stdout).unwrap_or_default();
        paths
            .lines()
            .map(str::trim)
            .find(|p| !p.contains("WindowsApps") && p.ends_with(".exe"))
            .map(str::to_string)
            .expect("No real python.exe found outside of WindowsApps")
    };
    #[cfg(not(windows))]
    let python_cmd: String = "python3".to_string();

    let python_script = format!(
        "import os; path_val = 'PRESENT' if os.environ.get('PATH') else 'MISSING'; f = open('{}', 'w'); f.write(f\"SECRET:{{os.environ.get('{}', 'SAFE')}}|ALLOWED:{{os.environ.get('ALLOWED_VAR', 'MISSING')}}|PATH:{{path_val}}\"); f.close()",
        output_path_str,
        secret_var
    );

    // 2. Spawn Python directly under the same isolated environment policy used by
    // external-process launchers. This keeps the test focused on env isolation
    // itself instead of RMCP startup/handshake behavior, which is unrelated.
    let mut cmd = Command::new(&python_cmd);
    cmd.arg("-c").arg(&python_script);
    apply_isolated_env(&mut cmd);
    cmd.env("ALLOWED_VAR", "explicit_value");

    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let output = cmd.output().unwrap_or_else(|e| {
        panic!(
            "Failed to spawn isolated python process '{}': {}",
            python_cmd, e
        )
    });

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!(
            "Isolated python process failed (status: {:?}) for {}.\nstdout: {}\nstderr: {}",
            output.status.code(),
            python_cmd,
            stdout.trim(),
            stderr.trim()
        );
    }

    assert!(
        output_file.exists(),
        "Output file was not created by isolated python process at {}",
        output_path_str
    );

    // 3. Check the file content
    let content = std::fs::read_to_string(&output_file).unwrap();
    println!("Captured Env Content: {}", content);

    // Before fix: SECRET:leaked_value|ALLOWED:explicit_value
    // After fix: SECRET:SAFE|ALLOWED:explicit_value

    assert!(
        !content.contains("SECRET:leaked_value"),
        "Env var SHOULD NOT be leaked after fix"
    );
    assert!(content.contains("SECRET:SAFE"), "Secret should be safe");
    assert!(
        content.contains("ALLOWED:explicit_value"),
        "Explicitly passed var SHOULD be present"
    );
    assert!(
        content.contains("PATH:PRESENT"),
        "Whitelisted PATH variable MUST be preserved after env_clear"
    );

    // Clean up
    unsafe { std::env::remove_var(secret_var) };
    let _ = std::fs::remove_file(output_file);
}
