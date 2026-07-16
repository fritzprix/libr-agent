use std::ffi::{OsStr, OsString};
#[cfg(unix)]
use std::io::IsTerminal;
use std::process::Command as StdCommand;
#[cfg(unix)]
use std::sync::OnceLock;
use tokio::process::Command as AsyncCommand;

#[cfg(unix)]
const PATH_CAPTURE_PREFIX: &str = "__LIBRAGENT_PATH_START__";
#[cfg(unix)]
const PATH_CAPTURE_SUFFIX: &str = "__LIBRAGENT_PATH_END__";

/// GUI-launched Unix apps frequently inherit a stripped PATH that omits user tool managers
/// like nvm, pnpm, uv, cargo, and pipx. Probe login shells once, cache the result, and merge
/// it back into isolated child environments.
#[cfg(unix)]
fn get_unix_shell_path() -> &'static str {
    static SHELL_PATH: OnceLock<String> = OnceLock::new();
    SHELL_PATH.get_or_init(probe_unix_shell_path)
}

#[cfg(unix)]
fn probe_unix_shell_path() -> String {
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

    let path_capture = format!("printf '{PATH_CAPTURE_PREFIX}%s{PATH_CAPTURE_SUFFIX}' \"$PATH\"");
    let is_cargo_test = std::env::var("CARGO_MANIFEST_DIR").is_ok();

    if !is_cargo_test {
        if let Some(source_script) =
            crate::utils::shell_runtime::build_unix_integration_source_script()
        {
            let targeted_probe = format!("{source_script}\n{path_capture}");
            if let Some(path) = run_unix_shell_path_probe(&shell, &["-l", "-c"], &targeted_probe) {
                return path;
            }
        }
    }

    let fallback_args: &[&[&str]] = if is_cargo_test {
        &[&["-l", "-c"]]
    } else if std::io::stdin().is_terminal() {
        &[&["-l", "-i", "-c"], &["-l", "-c"]]
    } else {
        &[&["-l", "-c"]]
    };

    for args in fallback_args {
        if let Some(path) = run_unix_shell_path_probe(&shell, args, &path_capture) {
            return path;
        }
    }

    String::new()
}

#[cfg(unix)]
fn run_unix_shell_path_probe(shell: &str, args: &[&str], probe_command: &str) -> Option<String> {
    let mut cmd = StdCommand::new(shell);
    cmd.args(args);
    cmd.arg(probe_command);

    let out = cmd.output().ok()?;
    if !out.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&out.stdout);
    let start = stdout.find(PATH_CAPTURE_PREFIX)?;
    let value_start = start + PATH_CAPTURE_PREFIX.len();
    let end_offset = stdout[value_start..].find(PATH_CAPTURE_SUFFIX)?;
    let captured = stdout[value_start..value_start + end_offset].trim();
    (!captured.is_empty()).then(|| captured.to_string())
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

// Discovered/shell paths are prepended so user-local tool dirs (e.g. Python Scripts)
// win over WindowsApps shims. Host PATH entries still follow and remain reachable.
fn merge_with_current_path(preferred: OsString, current_path: Option<OsString>) -> OsString {
    let merged = current_path
        .as_ref()
        .and_then(|current| merge_path_values(preferred.as_os_str(), current.as_os_str()))
        .unwrap_or(preferred);

    if merged.is_empty() {
        OsString::from(default_path())
    } else {
        merged
    }
}

pub fn get_effective_path_os() -> OsString {
    let current_path = std::env::var_os("PATH");

    #[cfg(unix)]
    {
        let shell_path = get_unix_shell_path();
        if !shell_path.is_empty() {
            return merge_with_current_path(OsString::from(shell_path), current_path);
        }
    }

    #[cfg(windows)]
    {
        if let Some(discovered) =
            crate::utils::windows_path_discovery::get_windows_discovered_path_os()
        {
            return merge_with_current_path(discovered, current_path);
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
    #[cfg(windows)]
    let platform_preserved = [
        "SystemRoot",
        "COMSPEC",
        "PATHEXT",
        "WINDIR",
        "APPDATA",
        "LOCALAPPDATA",
        "ProgramData",
        "ProgramFiles",
        "ProgramFiles(x86)",
        "CommonProgramFiles",
        "CommonProgramFiles(x86)",
        "USERPROFILE",
        "HOMEDRIVE",
        "HOMEPATH",
        "PSModulePath", // PowerShell module path (essential for PS tools)
    ];

    #[cfg(not(windows))]
    let platform_preserved = [
        "HOME",
        "USER",
        "LOGNAME",
        "DISPLAY",                  // GUI session (Unix)
        "WAYLAND_DISPLAY",          // GUI session (Unix)
        "DBUS_SESSION_BUS_ADDRESS", // GUI session (Unix)
    ];

    let common_preserved = [
        "PATH",
        "TEMP",
        "TMP",
        "TMPDIR",
        "TERM",
        "LANG",
        // Network Proxies (Crucial for CLI tools and MCP servers under VPN/Proxy)
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "NO_PROXY",
        "http_proxy",
        "https_proxy",
        "no_proxy",
    ];

    let dynamic_prefixes = ["GIT_", "CONDA_", "npm_config_"];

    let dynamic_exact = ["PYTHONPATH", "VIRTUAL_ENV"];

    let mut envs = Vec::new();
    for (key, value) in std::env::vars() {
        let is_preserved = {
            #[cfg(windows)]
            {
                common_preserved
                    .iter()
                    .any(|&p| p.eq_ignore_ascii_case(&key))
                    || platform_preserved
                        .iter()
                        .any(|&p| p.eq_ignore_ascii_case(&key))
                    || dynamic_exact.iter().any(|&p| p.eq_ignore_ascii_case(&key))
                    || dynamic_prefixes
                        .iter()
                        .any(|&p| key.len() >= p.len() && key[..p.len()].eq_ignore_ascii_case(p))
            }
            #[cfg(not(windows))]
            {
                common_preserved.contains(&key.as_str())
                    || platform_preserved.contains(&key.as_str())
                    || dynamic_exact.contains(&key.as_str())
                    || dynamic_prefixes.iter().any(|&p| key.starts_with(p))
            }
        };

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
