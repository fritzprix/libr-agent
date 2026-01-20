/// Test the improved command_helper utility
///
/// Run with: cargo run --example test_command_helper
use tauri_mcp_agent_lib::mcp::utils::command_helper;

fn main() {
    println!("=== Testing improved command_helper utility ===\n");

    // Test cases
    let test_cases = vec![
        ("npx", vec!["-y", "package"]),
        ("npm", vec!["install", "package"]),
        ("node", vec!["script.js"]),
        ("uvx", vec!["tool"]),
        ("python", vec!["script.py"]),
        ("custom.exe", vec!["arg1"]),
        ("unknown_command", vec!["arg1"]),
    ];

    for (command, args) in test_cases {
        let args_vec: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        let (final_cmd, final_args) = command_helper::prepare_command(command, &args_vec);

        println!("Input:  {} {:?}", command, args);
        println!("Output: {} {:?}", final_cmd, final_args);

        #[cfg(windows)]
        {
            let wrapped = final_cmd == "cmd.exe" || final_cmd == "powershell.exe";
            println!("Wrapped: {}", if wrapped { "✅ YES" } else { "❌ NO" });
        }

        #[cfg(not(windows))]
        {
            println!("Platform: Unix (no wrapping)");
        }

        println!();
    }

    println!("=== Testing with actual spawn ===\n");

    // Test actual spawn with npx
    println!("Testing: npx --version");
    let (cmd, args) = command_helper::prepare_command("npx", &["--version".to_string()]);

    match std::process::Command::new(&cmd).args(&args).output() {
        Ok(output) => {
            println!("✅ SUCCESS");
            println!("Command: {} {:?}", cmd, args);
            println!("Output: {}", String::from_utf8_lossy(&output.stdout).trim());
        }
        Err(e) => {
            println!("❌ FAILED: {}", e);
        }
    }

    println!("\n=== Summary ===");
    #[cfg(windows)]
    println!("Windows: Node.js/Python tools automatically wrapped with cmd.exe");

    #[cfg(not(windows))]
    println!("Unix/Linux: Commands pass through unchanged");
}
