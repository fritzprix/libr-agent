use std::process::Command as StdCommand;
use std::sync::OnceLock;
use tokio::process::Command as AsyncCommand;

/// On macOS, GUI apps launched via Finder/.app bundle inherit a minimal PATH from launchd
/// (typically `/usr/bin:/bin:/usr/sbin:/sbin`), stripping nvm, Homebrew, and other
/// user-installed tool managers.  This function runs the user's login shell once to
/// capture its full PATH and caches the result for subsequent calls.
#[cfg(target_os = "macos")]
fn get_macos_login_shell_path() -> &'static str {
    static SHELL_PATH: OnceLock<String> = OnceLock::new();
    SHELL_PATH.get_or_init(|| {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
        match StdCommand::new(&shell).args(["-l", "-c", "echo $PATH"]).output() {
            Ok(out) if out.status.success() => {
                String::from_utf8(out.stdout)
                    .unwrap_or_default()
                    .trim()
                    .to_string()
            }
            _ => String::new(),
        }
    })
}

/// Returns a list of environment variables that are safe to pass to external processes
/// (whitelisted essential system variables), preventing the leakage of host secrets.
pub fn get_isolated_env() -> Vec<(String, String)> {
    let preserved_vars = [
        "PATH",
        "SystemRoot",              // Windows
        "COMSPEC",                 // Windows
        "PATHEXT",                 // Windows
        "WINDIR",                  // Windows
        "APPDATA",                 // Windows
        "LOCALAPPDATA",            // Windows
        "ProgramData",             // Windows
        "ProgramFiles",            // Windows
        "ProgramFiles(x86)",       // Windows
        "CommonProgramFiles",      // Windows
        "CommonProgramFiles(x86)", // Windows
        "HOME",
        "USERPROFILE", // Windows
        "HOMEDRIVE",   // Windows
        "HOMEPATH",    // Windows
        "TEMP",
        "TMP",
        "TMPDIR",
        "TERM",
        "LANG",
        "DISPLAY",                  // GUI session (Unix)
        "WAYLAND_DISPLAY",          // GUI session (Unix)
        "DBUS_SESSION_BUS_ADDRESS", // GUI session (Unix)
    ];

    let mut envs = Vec::new();
    for (key, value) in std::env::vars() {
        #[cfg(windows)]
        let is_preserved = preserved_vars.iter().any(|&p| p.eq_ignore_ascii_case(&key));
        #[cfg(not(windows))]
        let is_preserved = preserved_vars.contains(&key.as_str());

        // XDG_RUNTIME_DIR exposes live D-Bus / Wayland sockets under /run/user/<uid>;
        // isolated processes have no business accessing them.
        if is_preserved
            || key.starts_with("LC_")
            || (key.starts_with("XDG_") && key != "XDG_RUNTIME_DIR")
        {
            envs.push((key, value));
        }
    }

    // On macOS, the PATH inherited by a GUI app is stripped of user tool-manager entries
    // (nvm, Homebrew, Volta, etc.).  Merge in the login-shell PATH so that commands like
    // `npx`, `node`, or `python` resolve correctly when spawning MCP server processes.
    #[cfg(target_os = "macos")]
    {
        let shell_path = get_macos_login_shell_path();
        if !shell_path.is_empty() {
            if let Some(entry) = envs.iter_mut().find(|(k, _)| k == "PATH") {
                let current = entry.1.clone();
                // shell_path wins on ordering; append any current entries not already present
                let mut parts: Vec<&str> = shell_path.split(':').collect();
                for part in current.split(':') {
                    if !part.is_empty() && !parts.contains(&part) {
                        parts.push(part);
                    }
                }
                entry.1 = parts.join(":");
            }
        }
    }

    envs
}

/// Scrubs the environment of the given command and applies the safe, isolated whitelist.
pub fn apply_isolated_env(cmd: &mut StdCommand) {
    cmd.env_clear();
    for (k, v) in get_isolated_env() {
        cmd.env(k, v);
    }
}

/// Async variant of `apply_isolated_env` for `tokio::process::Command`.
pub fn apply_isolated_env_async(cmd: &mut AsyncCommand) {
    cmd.env_clear();
    for (k, v) in get_isolated_env() {
        cmd.env(k, v);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_apply_isolated_env_clears_secrets() {
        // Set a dummy secret in the current process
        env::set_var("OPENAI_API_KEY", "secret-value");
        env::set_var("MY_PRIVATE_VAR", "private-value");

        let mut cmd = StdCommand::new("ls");
        apply_isolated_env(&mut cmd);

        // We can't directly inspect cmd.get_envs() easily in std::process::Command
        // so we check if get_isolated_env() contains our secrets
        let isolated = get_isolated_env();
        assert!(isolated.iter().all(|(k, _)| k != "OPENAI_API_KEY"));
        assert!(isolated.iter().all(|(k, _)| k != "MY_PRIVATE_VAR"));

        // Verify some essential vars are kept if they exist in the host
        if env::var("PATH").is_ok() {
            assert!(isolated.iter().any(|(k, _)| k == "PATH"));
        }
    }

    #[test]
    fn test_get_isolated_env_includes_whitelisted_prefixes() {
        env::set_var("LC_ALL", "en_US.UTF-8");
        env::set_var("XDG_CONFIG_HOME", "/tmp/config");
        env::set_var("XDG_RUNTIME_DIR", "/run/user/1000"); // Should be excluded

        let isolated = get_isolated_env();
        assert!(isolated.iter().any(|(k, _)| k == "LC_ALL"));
        assert!(isolated.iter().any(|(k, _)| k == "XDG_CONFIG_HOME"));
        assert!(isolated.iter().all(|(k, _)| k != "XDG_RUNTIME_DIR"));
    }
}
