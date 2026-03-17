use std::collections::HashMap;
use std::time::Duration;
use tauri_mcp_agent_lib::mcp::types::{MCPServerConfig, TransportConfig};
use tauri_mcp_agent_lib::mcp::SessionIsolationConfig;
use tauri_mcp_agent_lib::mcp::SessionMCPManager;

#[tokio::test]
async fn test_env_isolation_prevents_leakage() {
    // 1. Set a secret environment variable in the host process
    // We use a unique name to avoid conflicts
    let secret_var = "SECRET_LEAK_TEST_UUID_1234";
    unsafe { std::env::set_var(secret_var, "leaked_value") };

    // 2. Configure a manager to run a command that writes this variable to a file
    let mut configs = HashMap::new();
    let mut env_vars = HashMap::new();
    env_vars.insert("ALLOWED_VAR".to_string(), "explicit_value".to_string());

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

    configs.insert(
        "leak-tester".to_string(),
        MCPServerConfig {
            name: Some("leak-tester".to_string()),
            transport: TransportConfig::Stdio {
                command: python_cmd.to_string(),
                args: vec!["-c".to_string(), python_script],
                env: env_vars,
            },
            authentication: None,
            metadata: None,
        },
    );

    let config = SessionIsolationConfig {
        idle_timeout_minutes: 1,
        cleanup_interval_minutes: 1,
        process_startup_timeout_seconds: 10,
        max_restart_attempts: 0,
        http_connection_pool_size: 1,
    };

    let manager = SessionMCPManager::new(
        "test-session".to_string(),
        configs,
        config,
        std::env::current_dir().unwrap(),
    );

    // 3. Trigger spawn.
    // This will likely fail to connect as an MCP server, but the process should run.
    // We use list_tools which calls ensure_process_running internally.
    let _ = manager.list_tools("leak-tester").await;

    // Give it a moment to run - CI can be slow, so wait up to 5 seconds
    let mut found = false;
    for _ in 0..50 {
        if output_file.exists() {
            found = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // 4. Check the file content
    if !found {
        // If file doesn't exist, check if python is available. If not, skip test.
        let version_check = std::process::Command::new(&python_cmd)
            .arg("--version")
            .output();

        match version_check {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let stderr = String::from_utf8_lossy(&out.stderr);
                panic!(
                    "Output file not created after 5s. Python ({}) is available (stdout: {}, stderr: {}), but script failed to run or write to {}.",
                    python_cmd, stdout.trim(), stderr.trim(), output_path_str
                );
            }
            Err(e) => {
                panic!(
                    "{} not found ({}): this security env isolation test requires python to run",
                    python_cmd, e
                );
            }
        }
    }

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
