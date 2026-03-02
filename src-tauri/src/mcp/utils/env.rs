/// Returns a list of environment variables that are safe to pass to MCP servers
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
    ];

    let mut envs = Vec::new();
    for (key, value) in std::env::vars() {
        #[cfg(windows)]
        let is_preserved = preserved_vars.iter().any(|&p| p.eq_ignore_ascii_case(&key));
        #[cfg(not(windows))]
        let is_preserved = preserved_vars.contains(&key.as_str());

        if is_preserved || key.starts_with("LC_") || key.starts_with("XDG_") {
            envs.push((key, value));
        }
    }

    envs
}
