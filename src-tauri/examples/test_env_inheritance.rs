/// Standalone executable to verify environment variable inheritance in MCP server spawning
///
/// This test demonstrates that:
/// 1. tokio::process::Command inherits parent process environment by default
/// 2. cmd.env() adds/overrides specific variables without clearing inherited ones
/// 3. Commands like npx, npm, uvx work without full paths (using system PATH)
///
/// Run with: cargo run --example test_env_inheritance
use std::collections::HashMap;
use std::env;
use tokio::process::Command;

#[tokio::main]
async fn main() {
    println!("=== Environment Variable Inheritance Test ===\n");

    // Get current PATH from parent process
    let parent_path = env::var("PATH").expect("PATH not found in parent process");
    println!(
        "✓ Parent process PATH exists (length: {} bytes)",
        parent_path.len()
    );
    println!(
        "  First 200 chars: {}\n",
        &parent_path.chars().take(200).collect::<String>()
    );

    // Test 1: Spawn without env_clear() - should inherit PATH
    println!("Test 1: Spawning process WITHOUT env_clear()");
    test_with_env_inheritance().await;

    // Test 2: Spawn with env_clear() - should NOT inherit PATH
    println!("\nTest 2: Spawning process WITH env_clear()");
    test_without_env_inheritance().await;

    // Test 3: Test with custom env vars (simulating MCP config)
    println!("\nTest 3: Spawning with custom env vars (MCP pattern)");
    test_mcp_pattern().await;

    // Test 4: Try to run common commands (npx, npm, uvx, node, python)
    println!("\nTest 4: Testing common commands in PATH");
    test_common_commands().await;

    println!("\n=== All Tests Complete ===");
}

async fn test_with_env_inheritance() {
    // This simulates the pattern used in stdio_manager.rs
    let mut custom_env = HashMap::new();
    custom_env.insert("TEST_VAR".to_string(), "test_value".to_string());

    #[cfg(windows)]
    let cmd = Command::new("cmd.exe")
        .arg("/C")
        .arg("echo PATH=%PATH%")
        .env("TEST_VAR", "test_value") // Add custom var WITHOUT clearing
        .output()
        .await;

    #[cfg(not(windows))]
    let cmd = Command::new("sh")
        .arg("-c")
        .arg("echo PATH=$PATH")
        .env("TEST_VAR", "test_value")
        .output()
        .await;

    match cmd {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if stdout.contains("PATH=") && stdout.len() > 50 {
                println!(
                    "  ✓ SUCCESS: PATH inherited (output length: {} bytes)",
                    stdout.len()
                );
                println!(
                    "    First 200 chars: {}",
                    &stdout.chars().take(200).collect::<String>()
                );
            } else {
                println!("  ✗ FAILED: PATH not inherited");
                println!("    Output: {}", stdout);
            }
        }
        Err(e) => println!("  ✗ ERROR: {}", e),
    }
}

async fn test_without_env_inheritance() {
    #[cfg(windows)]
    let cmd = Command::new("cmd.exe")
        .arg("/C")
        .arg("echo PATH=%PATH%")
        .env_clear() // Clear all environment
        .env("TEST_VAR", "test_value")
        .output()
        .await;

    #[cfg(not(windows))]
    let cmd = Command::new("sh")
        .arg("-c")
        .arg("echo PATH=$PATH")
        .env_clear()
        .env("TEST_VAR", "test_value")
        .output()
        .await;

    match cmd {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if stdout.contains("PATH=%PATH%") || stdout.contains("PATH=") && stdout.len() < 50 {
                println!("  ✓ EXPECTED: PATH NOT inherited (cleared)");
                println!("    Output: {}", stdout.trim());
            } else {
                println!("  ✗ UNEXPECTED: PATH was inherited despite env_clear()");
            }
        }
        Err(e) => println!("  ✗ ERROR: {}", e),
    }
}

async fn test_mcp_pattern() {
    // Simulate the exact pattern from stdio_manager.rs
    let mut env_vars = HashMap::new();
    env_vars.insert("MCP_SERVER_NAME".to_string(), "test-server".to_string());
    env_vars.insert("MCP_SESSION_ID".to_string(), "test-session".to_string());

    #[cfg(windows)]
    let mut cmd = Command::new("cmd.exe");
    #[cfg(windows)]
    {
        cmd.arg("/C").arg("set");
        for (key, value) in &env_vars {
            cmd.env(key, value);
        }
    }

    #[cfg(not(windows))]
    let mut cmd = Command::new("sh");
    #[cfg(not(windows))]
    {
        cmd.arg("-c").arg("env");
        for (key, value) in &env_vars {
            cmd.env(key, value);
        }
    }

    match cmd.output().await {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let has_custom_vars =
                stdout.contains("MCP_SERVER_NAME") && stdout.contains("MCP_SESSION_ID");
            let has_path = stdout.contains("PATH");

            println!(
                "  Custom vars present: {}",
                if has_custom_vars { "✓" } else { "✗" }
            );
            println!(
                "  System PATH present: {}",
                if has_path { "✓" } else { "✗" }
            );

            if has_custom_vars && has_path {
                println!("  ✓ SUCCESS: MCP pattern works correctly");
            } else {
                println!("  ✗ FAILED: Missing expected environment variables");
            }
        }
        Err(e) => println!("  ✗ ERROR: {}", e),
    }
}

async fn test_common_commands() {
    let commands = vec![
        ("node", vec!["--version"]),
        ("npm", vec!["--version"]),
        ("npx", vec!["--version"]),
        ("python", vec!["--version"]),
        ("python3", vec!["--version"]),
        ("uvx", vec!["--version"]),
    ];

    for (cmd_name, args) in commands {
        let result = Command::new(cmd_name)
            .args(&args)
            .env("TEST_VAR", "test") // Add custom var to simulate MCP config
            .output()
            .await;

        match result {
            Ok(output) if output.status.success() => {
                let version = String::from_utf8_lossy(&output.stdout);
                println!("  ✓ {}: {}", cmd_name, version.trim());
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                println!("  ✗ {} failed: {}", cmd_name, stderr.trim());
            }
            Err(_) => {
                println!(
                    "  - {} not found in PATH (expected if not installed)",
                    cmd_name
                );
            }
        }
    }
}
