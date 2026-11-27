//! Applying a changed plugin environment to a shell that is already running
//! (#3008): `CwdChanged` hooks rewrite `CLAUDE_ENV_FILE` after the shell was
//! created, so the new values are exported into it.

use super::secrets::SecretMap;
use super::PersistentShellManager;
use log::warn;

/// Shell statements that take the environment from `before` to `after`. The
/// value a variable had in the shell before its first override is saved there,
/// and restored once no plugin sets it.
pub(super) fn env_sync_script(before: &SecretMap, after: &SecretMap, powershell: bool) -> String {
    let mut lines = Vec::new();
    for (name, value) in after {
        if before.get(name) == Some(value) {
            continue;
        }
        let saved = format!("__libragent_saved_{name}");
        if powershell {
            let value = value.replace('\'', "''");
            lines.push(format!(
                "if (-not (Test-Path variable:global:{saved})) {{ $global:{saved} = $env:{name} }}"
            ));
            lines.push(format!("$env:{name} = '{value}'"));
        } else {
            let value = value.replace('\'', r"'\''");
            lines.push(format!(
                "[ -n \"${{{saved}+x}}\" ] || {saved}=\"${{{name}-__libragent_unset__}}\""
            ));
            lines.push(format!("export {name}='{value}'"));
        }
    }
    for name in before.keys().filter(|name| !after.contains_key(*name)) {
        let saved = format!("__libragent_saved_{name}");
        if powershell {
            // Assigning `$null` removes an environment variable.
            lines.push(format!("$env:{name} = $global:{saved}"));
            lines.push(format!(
                "Remove-Variable -Scope Global -Name {saved} -ErrorAction SilentlyContinue"
            ));
        } else {
            lines.push(format!(
                "if [ \"${{{saved}-__libragent_unset__}}\" = __libragent_unset__ ]; then unset {name}; else export {name}=\"${saved}\"; fi; unset {saved}"
            ));
        }
    }
    lines.join("\n")
}

impl PersistentShellManager {
    /// Export the difference between two session environments into the running
    /// shell. A session without a shell gets the values when one is created.
    pub async fn sync_session_env(&self, session_id: &str, before: &SecretMap, after: &SecretMap) {
        let Some(shell) = self.running_shell(session_id).await else {
            return;
        };
        let mut shell = shell.lock().await;
        let script = env_sync_script(before, after, shell.uses_host_powershell_protocol());
        if script.is_empty() {
            return;
        }
        if let Err(error) = shell.execute(&script).await {
            warn!("plugin environment not applied to the shell of session {session_id}: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(entries: &[(&str, &str)]) -> SecretMap {
        entries
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn unchanged_environment_needs_no_script() {
        let env = map(&[("A", "1")]);
        assert!(env_sync_script(&env, &env, false).is_empty());
        assert!(env_sync_script(&env, &env, true).is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn script_overrides_then_restores_shell_values() {
        let before = map(&[]);
        let after = map(&[("KEPT", "it's new"), ("ADDED", "$(echo no)")]);
        let script = format!(
            "export KEPT=original\nunset ADDED\n{}\nprintf '%s|%s;' \"$KEPT\" \"$ADDED\"\n{}\nprintf '%s|%s' \"$KEPT\" \"${{ADDED-gone}}\"",
            env_sync_script(&before, &after, false),
            env_sync_script(&after, &before, false),
        );
        let output = std::process::Command::new("bash")
            .args(["-c", &script])
            .output()
            .expect("bash");
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            "it's new|$(echo no);original|gone"
        );
    }
}
