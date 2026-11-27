//! Command construction: the plugin environment a hook runs with
//! (`HookEnv`, #2926) and `${CLAUDE_PLUGIN_ROOT}` expansion.

use crate::services::hooks::schema::HookHandler;
use std::path::{Path, PathBuf};
use tokio::process::Command;

/// Environment a hook process runs with (#2926).
///
/// Before this, a hook saw only the plugin root aliases. The plugin variables
/// a Claude Code plugin can rely on are set here: `${CLAUDE_PLUGIN_DATA}`,
/// `CLAUDE_PROJECT_DIR`, `CLAUDE_ENV_FILE` (for `SessionStart` and `CwdChanged`), and one
/// `CLAUDE_PLUGIN_OPTION_<KEY>` per resolved `userConfig` value.
pub(super) struct HookEnv {
    pub(super) plugin_root: PathBuf,
    /// Per-plugin data directory, created before the hook runs.
    pub(super) data_dir: PathBuf,
    /// The session's workspace directory (`CLAUDE_PROJECT_DIR`).
    pub(super) project_dir: String,
    /// `CLAUDE_ENV_FILE`; only `SessionStart` and `CwdChanged` runs get one.
    pub(super) env_file: Option<PathBuf>,
    /// Resolved `userConfig` values, keyed by the manifest key.
    pub(super) options: std::collections::BTreeMap<String, String>,
}

impl HookEnv {
    /// Apply the plugin variables to a hook command. A data directory that
    /// cannot be created is logged and not fatal: the hook still runs, it just
    /// cannot write plugin data.
    pub(super) fn apply(&self, cmd: &mut Command) {
        let root = self.plugin_root.to_string_lossy();
        cmd.env("LIBRAGENT_PLUGIN_ROOT", root.as_ref());
        cmd.env("CLAUDE_PLUGIN_ROOT", root.as_ref());
        cmd.env("CURSOR_PLUGIN_ROOT", root.as_ref());

        if let Err(error) = crate::services::hooks::registry::ensure_plugin_data_dir(&self.data_dir)
        {
            log::warn!("CLAUDE_PLUGIN_DATA is unusable for this hook: {error}");
        }
        let data = self.data_dir.to_string_lossy();
        cmd.env("LIBRAGENT_PLUGIN_DATA", data.as_ref());
        cmd.env("CLAUDE_PLUGIN_DATA", data.as_ref());

        cmd.env("LIBRAGENT_PROJECT_DIR", &self.project_dir);
        cmd.env("CLAUDE_PROJECT_DIR", &self.project_dir);

        if let Some(env_file) = self.env_file.as_deref() {
            let path = env_file.to_string_lossy();
            cmd.env("LIBRAGENT_ENV_FILE", path.as_ref());
            cmd.env("CLAUDE_ENV_FILE", path.as_ref());
        }

        for (key, value) in &self.options {
            if let Some(name) = crate::services::skill_service::option_env_name(key) {
                cmd.env(name, value);
            }
        }
    }
}

pub(super) fn build_hook_command(env: &HookEnv, handler: &HookHandler) -> Result<Command, String> {
    let mut cmd = build_hook_shell_command(&env.plugin_root, handler)?;
    env.apply(&mut cmd);
    Ok(cmd)
}

fn build_hook_shell_command(plugin_root: &Path, handler: &HookHandler) -> Result<Command, String> {
    // Exec form (#3005): `args` present means the command is an executable and
    // each `args` element is one argument, with `${CLAUDE_PLUGIN_ROOT}`
    // substituted as a plain string. No shell runs, on any platform.
    if let Some(args) = handler.args.as_deref() {
        let program = expand_plugin_root_placeholders(&handler.command, plugin_root);
        let mut cmd = Command::new(&program);
        for arg in args {
            cmd.arg(expand_plugin_root_placeholders(arg, plugin_root));
        }
        cmd.current_dir(plugin_root);
        return Ok(cmd);
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(ps) = handler.powershell.as_deref() {
            let script = expand_plugin_root_placeholders(ps, plugin_root);
            let mut cmd = Command::new("powershell.exe");
            cmd.args(["-NoProfile", "-Command", &script]);
            cmd.current_dir(plugin_root);
            return Ok(cmd);
        }
        let script = expand_plugin_root_placeholders(&handler.command, plugin_root);
        let mut cmd = Command::new("cmd.exe");
        cmd.args(["/C", &script]);
        cmd.current_dir(plugin_root);
        Ok(cmd)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let script = expand_plugin_root_placeholders(&handler.command, plugin_root);
        let shell = handler.shell.as_deref().unwrap_or("bash");
        let mut cmd = if shell == "sh" {
            let mut c = Command::new("sh");
            c.args(["-c", &script]);
            c
        } else {
            let mut c = Command::new("bash");
            c.args(["-c", &script]);
            c
        };
        cmd.current_dir(plugin_root);
        Ok(cmd)
    }
}

fn expand_plugin_root_placeholders(command: &str, plugin_root: &Path) -> String {
    let root = plugin_root.to_string_lossy();
    command
        .replace("${LIBRAGENT_PLUGIN_ROOT}", &root)
        .replace("${CLAUDE_PLUGIN_ROOT}", &root)
        .replace("${CURSOR_PLUGIN_ROOT}", &root)
        .replace("$LIBRAGENT_PLUGIN_ROOT", &root)
        .replace("$CLAUDE_PLUGIN_ROOT", &root)
        .replace("$CURSOR_PLUGIN_ROOT", &root)
}
