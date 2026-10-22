use super::super::normalization;

/// What the caller asked the isolated shell to run.
#[derive(Debug, Clone, Copy)]
pub(crate) enum ShellInput<'a> {
    /// One-line command passed to `bash -c` after quote normalization.
    Command(&'a str),
    /// Multi-line script run verbatim (no normalization, no escaping by the caller).
    Script(&'a str),
}

impl<'a> ShellInput<'a> {
    /// Text the user wrote; used for shell policy, process listings, and responses.
    pub(crate) fn text(&self) -> &'a str {
        match self {
            Self::Command(text) | Self::Script(text) => text,
        }
    }

    /// Command line handed to the platform shell.
    pub(crate) fn executable(&self) -> String {
        match self {
            Self::Command(command) => normalization::normalize_shell_command(command),
            Self::Script(script) => wrap_script_for_bash(script),
        }
    }
}

/// Feed the script to `bash` through a quoted here-document on fd 3.
///
/// The quoted delimiter disables expansion, so the body runs byte-for-byte. Using fd 3 instead of
/// stdin keeps the script's own stdin free, so commands like `cat` or `read` cannot consume the
/// remaining script lines.
pub(crate) fn wrap_script_for_bash(script: &str) -> String {
    let body = script.strip_suffix('\n').unwrap_or(script);
    let mut delimiter = format!("LIBRAGENT_SCRIPT_{}", cuid2::create_id());
    while body.lines().any(|line| line == delimiter) {
        delimiter = format!("LIBRAGENT_SCRIPT_{}", cuid2::create_id());
    }
    format!("bash /dev/fd/3 3<<'{delimiter}'\n{body}\n{delimiter}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_input_is_not_normalized() {
        let script = "echo \"it's fine\"\n# don't add quotes\n";
        let executable = ShellInput::Script(script).executable();
        assert!(executable.starts_with("bash /dev/fd/3 3<<'LIBRAGENT_SCRIPT_"));
        assert!(executable.contains("echo \"it's fine\"\n# don't add quotes\nLIBRAGENT_SCRIPT_"));
        assert_eq!(ShellInput::Script(script).text(), script);
    }

    #[test]
    fn wrapped_script_ends_with_the_delimiter_line() {
        let wrapped = wrap_script_for_bash("echo a");
        let first_line = wrapped.lines().next().expect("first line");
        let delimiter = first_line
            .split("3<<'")
            .nth(1)
            .and_then(|rest| rest.strip_suffix('\''))
            .expect("delimiter");
        assert_eq!(wrapped.lines().last(), Some(delimiter));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn wrapped_script_runs_verbatim_and_keeps_stdin_free() {
        if !crate::utils::platform::command_exists("bash") {
            return;
        }
        let script = r#"for pr in 1 2; do echo "x ($(echo $pr | wc -l | tr -d ' ') lines)"; done
echo 'single "double" $HOME stays literal'
cat
echo after-cat
exit 3
"#;
        let output = tokio::process::Command::new("bash")
            .arg("-c")
            .arg(wrap_script_for_bash(script))
            .stdin(std::process::Stdio::null())
            .output()
            .await
            .expect("run bash");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert_eq!(
            stdout,
            "x (1 lines)\nx (1 lines)\nsingle \"double\" $HOME stays literal\nafter-cat\n"
        );
        assert_eq!(output.status.code(), Some(3));
    }
}
