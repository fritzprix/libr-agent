//! Non-interactive password support for POSIX persistent shells.
//!
//! The shell has no terminal, so nothing can type a password into it. Instead,
//! each POSIX shell (host bash or container sh) gets a private bin folder at the
//! front of `PATH`:
//!
//! - `askpass` prints `$SUDO_PASSWORD` (or exits 1 when it is empty) and is
//!   exported as `SUDO_ASKPASS`.
//! - `sudo` runs the real sudo with `-A` when `$SUDO_PASSWORD` is set. Without it,
//!   `SUDO_ASKPASS` is unset first: with `DISPLAY` set and no tty, sudo would run
//!   the askpass helper even without `-A`. Both calls force `LC_ALL=C` so sudo's
//!   own messages are English on any host locale — `password_hint` matches that
//!   text to add the `storeAs` guidance (#2970).
//! - `pkexec`, `su`, `doas` fail at once with a hint instead of hanging.
//!
//! The agent stores the password with `requireUserInput` + `storeAs=SUDO_PASSWORD`.

use tracing::{debug, warn};

use super::session::PersistentShell;

/// Hint for a stored `SUDO_PASSWORD` that sudo rejected.
pub const WRONG_SUDO_PASSWORD_HINT: &str =
    "The stored SUDO_PASSWORD is wrong. Ask again with storeAs=SUDO_PASSWORD to replace it.";

/// Hint for a command that waits on a password prompt the shell cannot answer.
pub const PASSWORD_PROMPT_HINT: &str = "This command needs a password that the shell cannot ask for. Use plain `sudo` (not `/usr/bin/sudo`, `su`, `doas`, or `pkexec`). Ask for the password with requireUserInput=true, inputType=password, storeAs=SUDO_PASSWORD, then run the command again in workspace__runInPersistentShell (one-off shells have no sudo helper).";

const BLOCKED_COMMANDS: [&str; 3] = ["pkexec", "su", "doas"];

/// Build the POSIX `sh` script that installs the askpass helper and wrappers.
pub(crate) fn build_install_script() -> String {
    let mut script = String::from(
        r#"if __lr_bin=$(mktemp -d "${TMPDIR:-/tmp}/libragent-bin.XXXXXX"); then
trap "rm -rf '$__lr_bin'" EXIT
cat > "$__lr_bin/askpass" <<'__LR_EOF'
#!/bin/sh
[ -n "$SUDO_PASSWORD" ] || exit 1
printf '%s\n' "$SUDO_PASSWORD"
__LR_EOF
__lr_sudo=$(command -v sudo 2>/dev/null || true)
if [ -n "$__lr_sudo" ]; then
cat > "$__lr_bin/sudo" <<__LR_EOF
#!/bin/sh
if [ -n "\$SUDO_PASSWORD" ]; then
  exec env LC_ALL=C "$__lr_sudo" -A "\$@"
fi
unset SUDO_ASKPASS
exec env LC_ALL=C "$__lr_sudo" "\$@"
__LR_EOF
fi
"#,
    );

    for name in BLOCKED_COMMANDS {
        script.push_str(&format!(
            "cat > \"$__lr_bin/{name}\" <<'__LR_EOF'\n#!/bin/sh\necho '{PASSWORD_PROMPT_HINT}' >&2\nexit 1\n__LR_EOF\n"
        ));
    }

    script.push_str(
        r#"chmod 700 "$__lr_bin"/*
export SUDO_ASKPASS="$__lr_bin/askpass"
export PATH="$__lr_bin:$PATH"
fi
unset __lr_bin __lr_sudo"#,
    );
    script
}

/// Return the guidance hint when command output shows a password problem.
pub fn password_hint(stdout: &str, stderr: &str) -> Option<&'static str> {
    let output = format!("{stdout}\n{stderr}").to_lowercase();
    if output.contains("incorrect password attempt") {
        return Some(WRONG_SUDO_PASSWORD_HINT);
    }
    let needs_prompt = output.contains("a terminal is required")
        || output.contains("no askpass program")
        || output.contains("no password was provided")
        || output.contains(&PASSWORD_PROMPT_HINT.to_lowercase());
    needs_prompt.then_some(PASSWORD_PROMPT_HINT)
}

impl PersistentShell {
    /// Install the askpass helper and wrappers in POSIX shells.
    pub(crate) async fn install_sudo_shims(&mut self) {
        if self.uses_host_powershell_protocol() {
            return;
        }
        match self.execute(&build_install_script()).await {
            Ok((_, _, 0, _)) => {
                debug!("Installed sudo shims for session {}", self.session_id());
            }
            Ok((_, stderr, exit_code, _)) => {
                warn!(
                    "Sudo shim install exited with code {exit_code} for session {}: {}",
                    self.session_id(),
                    stderr.trim()
                );
            }
            Err(error) => {
                warn!(
                    "Sudo shim install failed for session {}: {error}",
                    self.session_id()
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrong_password_hint_wins() {
        let stderr = "sudo: 1 incorrect password attempt";
        assert_eq!(password_hint("", stderr), Some(WRONG_SUDO_PASSWORD_HINT));
    }

    #[test]
    fn missing_terminal_or_askpass_gives_prompt_hint() {
        for stderr in [
            "sudo: a terminal is required to read the password",
            "sudo: no askpass program specified, try setting SUDO_ASKPASS",
            "sudo: no password was provided",
        ] {
            assert_eq!(password_hint("", stderr), Some(PASSWORD_PROMPT_HINT));
        }
    }

    #[test]
    fn unrelated_output_gives_no_hint() {
        assert_eq!(password_hint("ok", "permission denied"), None);
    }

    #[test]
    fn hint_has_no_single_quote() {
        // The hint is embedded in single-quoted `echo` inside the wrappers.
        assert!(!PASSWORD_PROMPT_HINT.contains('\''));
    }

    #[cfg(unix)]
    #[test]
    fn install_script_creates_working_wrappers() {
        let script = format!(
            "{}\nprintf '%s\\n' \"$SUDO_ASKPASS\"\npkexec true; echo \"code=$?\"\nSUDO_PASSWORD=dummy \"$SUDO_ASKPASS\"",
            build_install_script()
        );
        let output = std::process::Command::new("sh")
            .arg("-c")
            .arg(&script)
            .output()
            .expect("run sh");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stdout.contains("libragent-bin."), "stdout: {stdout}");
        assert!(stdout.contains("code=1"), "stdout: {stdout}");
        assert!(stdout.trim_end().ends_with("dummy"), "stdout: {stdout}");
        assert!(stderr.contains(PASSWORD_PROMPT_HINT), "stderr: {stderr}");
    }

    #[cfg(unix)]
    #[test]
    fn sudo_wrapper_passes_askpass_only_with_a_stored_password() {
        // A fake `sudo` first in PATH stands in for the real one the wrapper resolves.
        let script = format!(
            r#"__fake=$(mktemp -d)
cat > "$__fake/sudo" <<'EOF'
#!/bin/sh
echo "args=$* askpass=${{SUDO_ASKPASS-unset}}"
EOF
chmod 700 "$__fake/sudo"
PATH="$__fake:$PATH"
{install}
SUDO_PASSWORD=dummy sudo true
SUDO_PASSWORD= sudo true
SUDO_PASSWORD= "$SUDO_ASKPASS"; echo "askpass_code=$?"
rm -rf "$__fake""#,
            install = build_install_script()
        );
        let output = std::process::Command::new("sh")
            .arg("-c")
            .arg(&script)
            .output()
            .expect("run sh");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = stdout.lines().collect();
        assert_eq!(lines.len(), 3, "stdout: {stdout}");
        assert!(
            lines[0].starts_with("args=-A true askpass=") && lines[0].contains("libragent-bin."),
            "stdout: {stdout}"
        );
        assert_eq!(lines[1], "args=true askpass=unset", "stdout: {stdout}");
        assert_eq!(lines[2], "askpass_code=1", "stdout: {stdout}");
    }

    /// Runs `run` with the shims installed and a fake real `sudo` whose stderr is
    /// returned. `fake_sudo_body` is the body of that fake sudo script.
    #[cfg(unix)]
    fn run_with_fake_sudo(fake_sudo_body: &str, run: &str) -> String {
        let script = format!(
            r#"__fake=$(mktemp -d)
cat > "$__fake/sudo" <<'__FAKE_EOF'
#!/bin/sh
{fake_sudo_body}
__FAKE_EOF
chmod 700 "$__fake/sudo"
PATH="$__fake:$PATH"
{install}
{run}
rm -rf "$__fake""#,
            fake_sudo_body = fake_sudo_body,
            install = build_install_script(),
            run = run,
        );
        let output = std::process::Command::new("sh")
            .arg("-c")
            .arg(&script)
            .output()
            .expect("run sh");
        String::from_utf8_lossy(&output.stderr).to_string()
    }

    /// A stand-in for a gettext-localized sudo: it answers in Korean unless the
    /// locale category says C, like sudo on a ko_KR host (#2970).
    #[cfg(unix)]
    const LOCALIZED_SUDO_BODY: &str = r#"if [ "$LC_ALL" = C ]; then
  echo "sudo: a terminal is required to read the password" >&2
else
  echo "sudo: 암호를 읽으려면 터미널이 필요합니다" >&2
fi
exit 1"#;

    #[cfg(unix)]
    #[test]
    fn sudo_wrapper_forces_english_messages_on_a_non_english_host_locale() {
        let stderr = run_with_fake_sudo(LOCALIZED_SUDO_BODY, "SUDO_PASSWORD= sudo whoami");
        assert!(
            stderr.contains("a terminal is required"),
            "sudo must answer in English: {stderr}"
        );
        assert!(
            !stderr.contains("암호를"),
            "localized text leaked: {stderr}"
        );
        assert_eq!(password_hint("", &stderr), Some(PASSWORD_PROMPT_HINT));
    }

    /// Localized sudo on the wrong-stored-password path: `sudo -A` reports the
    /// failed attempt in Korean unless forced to English (#2970).
    #[cfg(unix)]
    const LOCALIZED_WRONG_PASSWORD_SUDO_BODY: &str = r#"if [ "$LC_ALL" = C ]; then
  echo "sudo: 1 incorrect password attempt" >&2
else
  echo "sudo: 인증 시도 1회 실패" >&2
fi
exit 1"#;

    #[cfg(unix)]
    #[test]
    fn wrong_stored_password_is_reported_in_english_on_a_non_english_host_locale() {
        let stderr = run_with_fake_sudo(
            LOCALIZED_WRONG_PASSWORD_SUDO_BODY,
            "SUDO_PASSWORD=stale sudo whoami",
        );
        assert!(
            stderr.contains("1 incorrect password attempt"),
            "sudo must answer in English: {stderr}"
        );
        assert!(!stderr.contains("인증"), "localized text leaked: {stderr}");
        assert_eq!(password_hint("", &stderr), Some(WRONG_SUDO_PASSWORD_HINT));
    }
}
