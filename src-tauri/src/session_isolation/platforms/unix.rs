use crate::session_isolation::types::{IsolatedProcessConfig, ShellType};
use tokio::process::Command as AsyncCommand;
use tracing::{info, warn};

/// Basic isolation: environment variables and working directory
pub async fn create_basic_isolated_command(
    config: IsolatedProcessConfig,
) -> Result<AsyncCommand, String> {
    // Unix-like logic from original create_basic_isolated_command
    let shell_type = config.shell_type.unwrap_or(ShellType::Bash);

    // Windows-specific shells are not supported on Unix platforms.
    if shell_type.is_windows() {
        return Err(
            "Windows shells (PowerShell/Cmd) are not supported on Unix systems. Use Bash."
                .to_string(),
        );
    }

    let mut cmd = AsyncCommand::new(shell_type.command());

    // Set working directory
    cmd.current_dir(&config.workspace_path);

    // Apply environment isolation: clear all inherited environment variables so that
    // host-level secrets (e.g., API keys, tokens, credentials) are not exposed inside
    // the isolated shell process. We then explicitly re-add only a small, trusted
    // whitelist of system variables required for basic shell and terminal behavior.
    cmd.env_clear();

    // Whitelist essential variables from parent environment.
    // DISPLAY and XAUTHORITY are intentionally excluded to prevent GUI/X11 access
    // from within the isolated shell (screen capture, input injection, etc.)
    let preserved_vars = [
        "TERM",
        "USER",
        "LOGNAME",
        "SHELL",
        "HOME",
        "http_proxy",
        "https_proxy",
        "no_proxy",
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "NO_PROXY",
    ];

    for key in &preserved_vars {
        if let Ok(val) = std::env::var(key) {
            cmd.env(key, val);
        }
    }

    cmd.env("PATH", crate::utils::env::get_effective_path_os());

    // Preserve locale and XDG base directory variables for consistency with MCP stdio isolation.
    // This includes all LC_* variables (beyond LC_ALL/LANG) and XDG_* variables.
    // LC_ALL and LANG are still explicitly overridden below to enforce consistent English output.
    // XDG_RUNTIME_DIR is explicitly blocked: it exposes live D-Bus / Wayland sockets
    // under /run/user/<uid> which an isolated shell has no business accessing.
    for (key, value) in std::env::vars() {
        if key == "XDG_RUNTIME_DIR" {
            continue; // Block D-Bus / Wayland socket exposure
        }
        if key.starts_with("LC_") || key.starts_with("XDG_") {
            cmd.env(&key, value);
        }
    }

    // Keep the host home directory for tool config discovery, but pin shell state to the workspace.
    let tmp_dir = config.workspace_path.join(".libragent/tmp");
    tokio::fs::create_dir_all(&tmp_dir)
        .await
        .map_err(|e| format!("Failed to create tmp dir: {}", e))?;
    cmd.env("PWD", &config.workspace_path);
    cmd.env("TMPDIR", &tmp_dir);
    // Force English output for consistent AI reasoning
    cmd.env("LC_ALL", "C.UTF-8");
    cmd.env("LANG", "C.UTF-8");

    // Add user-specified environment variables (applies to all platforms)
    for (key, value) in &config.env_vars {
        cmd.env(key, value);
    }

    // Unix shells (bash, sh) use -c flag.
    // Instead of string concatenation which is vulnerable to injection if args contain metacharacters,
    // we use the `sh -c "$@"` pattern to pass arguments safely.
    // Syntax: sh -c 'command "$@"' -- arg0 arg1 arg2...
    // where arg0 becomes $0 (typically script name), arg1 becomes $1, etc.

    // We set $0 to the command name itself for better `ps` output visibility if supported,
    // or just a placeholder. Let's use config.command as $0.

    // Command string: simply execute "$@" which expands to all positional parameters preserving quoting.
    // Wait, "$@" expands to $1 $2... it does NOT include $0.
    // So if we run `sh -c '"$@"' -- cmd arg1`, it runs `arg1`!

    let mut shell_args = vec!["-c"];

    if config.args.is_empty() {
        // Run as a script: sh -c "ls -l"
        shell_args.push(&config.command);
    } else {
        // Run as executable + args safely: sh -c '"$0" "$@"' exe arg1 arg2
        shell_args.push("\"$0\" \"$@\"");
        shell_args.push(&config.command); // $0
        for arg in &config.args {
            shell_args.push(arg); // $1, $2...
        }
    }

    info!(
        "Unix shell execution: {} {:?} (original: '{}' args: {:?})",
        shell_type.command(),
        shell_args,
        config.command,
        config.args
    );
    cmd.args(shell_args);

    info!(
        "Isolated command created for session {} with isolation level {:?}",
        config.session_id, config.isolation_level
    );
    Ok(cmd)
}

/// Medium isolation: process groups
pub async fn create_medium_isolated_command(
    config: IsolatedProcessConfig,
) -> Result<AsyncCommand, String> {
    let mut cmd = create_basic_isolated_command(config.clone()).await?;

    // Apply platform-specific process group isolation
    #[cfg(unix)]
    {
        cmd.process_group(0); // Create new process group
    }

    Ok(cmd)
}

/// High isolation fallback for generic Unix: medium isolation + warning
#[allow(dead_code)] // Unused on Linux/macOS as they have specific implementations
pub async fn create_high_isolated_command(
    config: IsolatedProcessConfig,
) -> Result<AsyncCommand, String> {
    warn!("High isolation not supported on this platform, falling back to medium isolation");
    create_medium_isolated_command(config).await
}
