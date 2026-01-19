/// Test to verify that wrapping npx with cmd.exe /c fixes Windows spawn issues
///
/// Run with: cargo run --example test_cmd_exe_wrapper
///
/// This test is Windows-specific. On Unix/Linux systems, commands like npx
/// work directly without shell wrapping.
use std::process::{Command, Stdio};

fn main() {
    println!("=== Testing Windows cmd.exe wrapper for .cmd files ===\n");

    // Test 1: Direct npx execution (will likely fail on Windows)
    println!("Test 1: Direct 'npx' command");
    match Command::new("npx")
        .arg("--version")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => {
            let output = child.wait_with_output().unwrap();
            println!("✅ SUCCESS");
            println!("stdout: {}", String::from_utf8_lossy(&output.stdout));
        }
        Err(e) => {
            println!("❌ FAILED: {}", e);
        }
    }

    println!("\n---\n");

    // Test 2: cmd.exe /c npx (should work on Windows)
    println!("Test 2: 'cmd.exe /c npx' command");
    match Command::new("cmd.exe")
        .arg("/c")
        .arg("npx")
        .arg("--version")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => {
            let output = child.wait_with_output().unwrap();
            println!("✅ SUCCESS");
            println!("stdout: {}", String::from_utf8_lossy(&output.stdout));
        }
        Err(e) => {
            println!("❌ FAILED: {}", e);
        }
    }

    println!("\n---\n");

    // Test 3: Simulate MCP server spawn with rpg-mcp-server
    println!("Test 3: cmd.exe /c npx -y rpg-mcp-server");
    match Command::new("cmd.exe")
        .arg("/c")
        .arg("npx")
        .arg("-y")
        .arg("rpg-mcp-server")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(mut child) => {
            // Wait a bit to see startup output
            std::thread::sleep(std::time::Duration::from_secs(2));

            // Try to read output (non-blocking)
            println!("✅ Process spawned successfully");
            println!("Process ID: {}", child.id());

            // Kill the process after verification
            let _ = child.kill();
            println!("Process terminated for testing");
        }
        Err(e) => {
            println!("❌ FAILED: {}", e);
        }
    }

    println!("\n=== Summary ===");
    println!("On Windows, .cmd files (like npx.cmd) require cmd.exe wrapper");
    println!("Direct Command::new(\"npx\") will fail with 'program not found'");
    println!("Solution: Command::new(\"cmd.exe\").args([\"/c\", \"npx\", ...])");
}
