use std::process::Command;

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

    envs
}

/// Scrubs the environment of the given command and applies the safe, isolated whitelist.
pub fn apply_isolated_env(cmd: &mut Command) {
    cmd.env_clear();
    for (k, v) in get_isolated_env() {
        cmd.env(k, v);
    }
}
