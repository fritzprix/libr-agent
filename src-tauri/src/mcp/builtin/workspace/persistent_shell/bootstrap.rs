//! Opt-in runtime bootstrap for persistent shells.
//!
//! When enabled via settings, sources only known integration scripts (conda.sh,
//! nvm.sh) without loading full shell rc files or running `conda activate`.

use tracing::{debug, warn};

use super::session::PersistentShell;

impl PersistentShell {
    /// Source conda/nvm integration scripts when the opt-in setting is enabled.
    pub async fn apply_runtime_bootstrap(&mut self) {
        // Docker attach on Windows hosts uses bash/sh inside the container. Host
        // PowerShell conda/nvm bootstrap must not run there (and unix host scripts
        // are irrelevant inside the image).
        if !self.uses_host_powershell_protocol() {
            #[cfg(unix)]
            let command = build_unix_bootstrap_command();
            #[cfg(windows)]
            let command: Option<String> = None;
            let Some(command) = command else {
                debug!(
                    "Skipping shell runtime bootstrap for session {}: no conda/nvm integration found (or container shell)",
                    self.session_id()
                );
                return;
            };
            self.run_bootstrap_command(&command).await;
            return;
        }

        #[cfg(windows)]
        let command = build_windows_bootstrap_command();
        #[cfg(unix)]
        let command: Option<String> = None;

        let Some(command) = command else {
            debug!(
                "Skipping shell runtime bootstrap for session {}: no conda/nvm integration found",
                self.session_id()
            );
            return;
        };

        self.run_bootstrap_command(&command).await;
    }

    async fn run_bootstrap_command(&mut self, command: &str) {
        match self.execute(command).await {
            Ok((stdout, stderr, 0, _cwd)) => {
                debug!(
                    "Shell runtime bootstrap completed for session {} (stdout: {} bytes, stderr: {} bytes)",
                    self.session_id(),
                    stdout.len(),
                    stderr.len()
                );
            }
            Ok((stdout, stderr, exit_code, _cwd)) => {
                warn!(
                    "Shell runtime bootstrap exited with code {} for session {} (stdout: {}, stderr: {})",
                    exit_code,
                    self.session_id(),
                    stdout.trim(),
                    stderr.trim()
                );
            }
            Err(error) => {
                warn!(
                    "Shell runtime bootstrap failed for session {}: {error}",
                    self.session_id()
                );
            }
        }
    }
}

#[cfg(unix)]
fn build_unix_bootstrap_command() -> Option<String> {
    let mut command = crate::utils::shell_runtime::build_unix_integration_source_script()?;
    command.push_str("\nunset PYTHONPATH LD_LIBRARY_PATH 2>/dev/null || true");
    Some(command)
}

#[cfg(windows)]
fn build_windows_bootstrap_command() -> Option<String> {
    use crate::utils::shell_runtime::{
        discover_conda_path_prefixes, discover_nvm_home, powershell_single_quote,
    };

    let conda_paths = discover_conda_path_prefixes();
    let nvm_home = discover_nvm_home();

    if conda_paths.is_empty() && nvm_home.is_none() {
        return None;
    }

    let mut parts = Vec::new();

    for path in conda_paths {
        let quoted = powershell_single_quote(&path.to_string_lossy());
        parts.push(format!(
            "if (Test-Path {quoted}) {{ $env:Path = {quoted} + ';' + $env:Path }}"
        ));
    }

    if let Some(nvm_home) = nvm_home {
        let quoted = powershell_single_quote(&nvm_home.to_string_lossy());
        parts.push(format!(
            "if (Test-Path {quoted}) {{ $env:Path = {quoted} + ';' + $env:Path }}"
        ));
    }

    parts.push("Remove-Item Env:PYTHONPATH -ErrorAction SilentlyContinue".to_string());

    Some(parts.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn unix_bootstrap_command_unsets_pythonpath() {
        if let Some(command) = build_unix_bootstrap_command() {
            assert!(
                command.contains("unset PYTHONPATH LD_LIBRARY_PATH"),
                "bootstrap should strip risky env vars"
            );
        }
    }

    #[cfg(windows)]
    #[test]
    fn windows_bootstrap_prepends_paths_without_activate() {
        if let Some(command) = build_windows_bootstrap_command() {
            assert!(!command.contains("conda activate"));
            assert!(command.contains("Remove-Item Env:PYTHONPATH"));
        }
    }
}
