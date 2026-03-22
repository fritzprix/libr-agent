use std::ffi::{OsStr, OsString};
use std::process::Command as StdCommand;
#[cfg(unix)]
use std::sync::OnceLock;
use tokio::process::Command as AsyncCommand;

#[cfg(unix)]
const PATH_CAPTURE_PREFIX: &str = "__LIBRAGENT_PATH_START__";
#[cfg(unix)]
const PATH_CAPTURE_SUFFIX: &str = "__LIBRAGENT_PATH_END__";

/// GUI-launched Unix apps frequently inherit a stripped PATH that omits user tool managers
/// like nvm, pnpm, uv, cargo, and pipx. Probe an interactive login shell once, cache the
/// result, and merge it back into isolated child environments.
#[cfg(unix)]
fn get_unix_shell_path() -> &'static str {
    static SHELL_PATH: OnceLock<String> = OnceLock::new();
    SHELL_PATH.get_or_init(|| {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| {
            #[cfg(target_os = "macos")]
            {
                "/bin/zsh".to_string()
            }

            #[cfg(not(target_os = "macos"))]
            {
                "/bin/bash".to_string()
            }
        });

        let probe_args = [["-l", "-i", "-c"], ["-l", "-c", ""]];
        let probe_command =
            format!("printf '{PATH_CAPTURE_PREFIX}%s{PATH_CAPTURE_SUFFIX}' \"$PATH\"");

        for args in probe_args {
            let mut cmd = StdCommand::new(&shell);
            if args[2].is_empty() {
                cmd.args([args[0], args[1], &probe_command]);
            } else {
                cmd.args([args[0], args[1], args[2], &probe_command]);
            }

            if let Ok(out) = cmd.output() {
                if !out.status.success() {
                    continue;
                }

                let stdout = String::from_utf8_lossy(&out.stdout);
                if let Some(start) = stdout.find(PATH_CAPTURE_PREFIX) {
                    let value_start = start + PATH_CAPTURE_PREFIX.len();
                    if let Some(end_offset) = stdout[value_start..].find(PATH_CAPTURE_SUFFIX) {
                        let captured = stdout[value_start..value_start + end_offset].trim();
                        if !captured.is_empty() {
                            return captured.to_string();
                        }
                    }
                }
            }
        }

        String::new()
    })
}

fn default_path() -> &'static str {
    #[cfg(windows)]
    {
        "C:\\Windows\\System32;C:\\Windows;C:\\Windows\\System32\\WindowsPowerShell\\v1.0"
    }

    #[cfg(not(windows))]
    {
        "/usr/local/bin:/usr/bin:/bin:/usr/local/sbin:/usr/sbin:/sbin"
    }
}

fn merge_path_values(preferred: &OsStr, fallback: &OsStr) -> Option<OsString> {
    let mut merged = Vec::new();

    for source in [preferred, fallback] {
        for path in std::env::split_paths(source) {
            if path.as_os_str().is_empty() || merged.iter().any(|existing| existing == &path) {
                continue;
            }
            merged.push(path);
        }
    }

    if merged.is_empty() {
        None
    } else {
        std::env::join_paths(merged).ok()
    }
}

pub fn get_effective_path_os() -> OsString {
    let current_path = std::env::var_os("PATH");

    #[cfg(unix)]
    {
        let shell_path = get_unix_shell_path();
        if !shell_path.is_empty() {
            let preferred = OsString::from(shell_path);
            let merged = current_path
                .as_ref()
                .and_then(|current| merge_path_values(preferred.as_os_str(), current.as_os_str()))
                .unwrap_or(preferred);

            if !merged.is_empty() {
                return merged;
            }
        }
    }

    current_path.unwrap_or_else(|| OsString::from(default_path()))
}

pub fn get_effective_path() -> String {
    get_effective_path_os().to_string_lossy().into_owned()
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

    let effective_path = get_effective_path();
    if let Some(entry) = envs.iter_mut().find(|(k, _)| k == "PATH") {
        entry.1 = effective_path;
    } else {
        envs.push(("PATH".to_string(), effective_path));
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
        assert!(isolated.iter().any(|(k, _)| k == "PATH"));
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

    #[test]
    fn test_merge_path_values_deduplicates_and_preserves_order() {
        let merged = merge_path_values(
            OsStr::new("/opt/custom/bin:/usr/bin"),
            OsStr::new("/usr/bin:/bin"),
        )
        .expect("merged path");

        let parts = std::env::split_paths(&merged)
            .map(|path| path.to_string_lossy().to_string())
            .collect::<Vec<_>>();

        assert_eq!(parts, vec!["/opt/custom/bin", "/usr/bin", "/bin"]);
    }
}
