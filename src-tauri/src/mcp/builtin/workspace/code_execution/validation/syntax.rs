//! Pre-execution bash syntax check (`bash -n`) so broken quoting fails before anything runs.

#[cfg_attr(not(unix), allow(dead_code))] // only `check_bash_syntax` on unix calls the parser
const SNIPPET_MAX_CHARS: usize = 120;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BashSyntaxError {
    /// 1-based line reported by bash (for EOF errors this is the end of the command).
    pub line: Option<usize>,
    /// Parser message without the `bash: -c: line N:` prefix.
    pub detail: String,
    /// Quote character bash was still looking for, when the error is an unclosed quote.
    pub unmatched_quote: Option<char>,
    /// The offending source line (echoed by bash or taken from the command).
    pub snippet: Option<String>,
}

/// Runs `bash -n -c <command>` and returns the parse error, if any.
///
/// Fails open (returns `None`) when bash is unavailable, times out, or cannot be spawned:
/// this is a usability check, not a security boundary.
#[cfg(unix)]
pub async fn check_bash_syntax(command: &str) -> Option<BashSyntaxError> {
    use std::process::Stdio;
    use std::time::Duration;

    const CHECK_TIMEOUT: Duration = Duration::from_secs(3);

    let child = tokio::process::Command::new("bash")
        .arg("-n")
        .arg("-c")
        .arg(command)
        .env("LC_ALL", "C")
        .env_remove("BASH_ENV")
        .env_remove("ENV")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .ok()?;

    let output = tokio::time::timeout(CHECK_TIMEOUT, child.wait_with_output())
        .await
        .ok()?
        .ok()?;
    if output.status.success() {
        return None;
    }
    Some(parse_bash_syntax_error(
        command,
        &String::from_utf8_lossy(&output.stderr),
    ))
}

#[cfg(not(unix))]
pub async fn check_bash_syntax(_command: &str) -> Option<BashSyntaxError> {
    None
}

/// Strip `bash: -c: line N: ` and return `(N, rest)`.
#[cfg_attr(not(unix), allow(dead_code))]
fn split_line_prefix(line: &str) -> Option<(usize, &str)> {
    let idx = line.find("line ")?;
    let after = &line[idx + "line ".len()..];
    let (number, rest) = after.split_once(':')?;
    let number = number.trim().parse::<usize>().ok()?;
    Some((number, rest.trim_start()))
}

#[cfg_attr(not(unix), allow(dead_code))]
fn truncate_chars(text: &str, max: usize) -> String {
    let mut chars = text.chars();
    let head: String = chars.by_ref().take(max).collect();
    if chars.next().is_some() {
        format!("{head}...")
    } else {
        head
    }
}

#[cfg_attr(not(unix), allow(dead_code))]
pub(super) fn parse_bash_syntax_error(command: &str, stderr: &str) -> BashSyntaxError {
    let mut line = None;
    let mut detail = None;
    let mut echoed_source = None;

    for raw in stderr.lines() {
        let Some((number, rest)) = split_line_prefix(raw) else {
            continue;
        };
        // bash echoes the offending source line as `` `source' `` after "syntax error near ...".
        if detail.is_some() && rest.starts_with('`') && rest.ends_with('\'') && rest.len() >= 2 {
            echoed_source = Some(rest[1..rest.len() - 1].to_string());
            break;
        }
        if detail.is_none() {
            line = Some(number);
            detail = Some(rest.to_string());
        }
    }

    let detail = detail.unwrap_or_else(|| {
        let trimmed = stderr.trim();
        if trimmed.is_empty() {
            "bash reported a syntax error".to_string()
        } else {
            trimmed.to_string()
        }
    });

    let unmatched_quote = detail
        .split_once("looking for matching `")
        .and_then(|(_, rest)| rest.chars().next());

    let snippet = echoed_source
        .or_else(|| {
            let lines: Vec<&str> = command.lines().collect();
            let index = line?.checked_sub(1)?.min(lines.len().checked_sub(1)?);
            Some(lines[index].trim().to_string())
        })
        .filter(|s| !s.is_empty())
        .map(|s| truncate_chars(&s, SNIPPET_MAX_CHARS));

    BashSyntaxError {
        line,
        detail,
        unmatched_quote,
        snippet,
    }
}

impl BashSyntaxError {
    pub fn summary(&self) -> String {
        let location = self
            .line
            .map(|n| format!(" at line {n}"))
            .unwrap_or_default();
        let mut message = format!(
            "Shell syntax error{location}: {}. The command was not executed.",
            self.detail
        );
        if let Some(snippet) = &self.snippet {
            message.push_str(&format!("\nNear: {snippet}"));
        }
        message
    }

    pub fn guidance(&self) -> Vec<String> {
        let mut guidance = Vec::new();
        if let Some(quote) = self.unmatched_quote {
            let kind = match quote {
                '"' => "double quote (\")".to_string(),
                '\'' => "single quote (')".to_string(),
                '`' => "backtick (`)".to_string(),
                other => format!("'{other}'"),
            };
            guidance.push(format!(
                "A {kind} is opened but never closed. bash reports the line where the input ended, so check earlier lines and $( ) substitutions for the opening quote."
            ));
        }
        guidance.extend(super::guidance::write_file_then_shell_guidance());
        guidance
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ISSUE_EXAMPLE: &str =
        r#"for pr in 1 2; do echo "x ($(gh pr diff $pr | wc -l) lines); done"#;

    #[test]
    fn parses_unclosed_double_quote() {
        let err = parse_bash_syntax_error(
            ISSUE_EXAMPLE,
            "bash: -c: line 1: unexpected EOF while looking for matching `\"'\n",
        );
        assert_eq!(err.line, Some(1));
        assert_eq!(err.unmatched_quote, Some('"'));
        assert_eq!(err.snippet.as_deref(), Some(ISSUE_EXAMPLE));
        assert!(err.summary().contains("at line 1"));
        assert!(err.guidance()[0].contains("double quote"));
    }

    #[test]
    fn clamps_eof_line_to_last_command_line() {
        let err = parse_bash_syntax_error(
            "echo ok\nls \"a",
            "bash: -c: line 3: unexpected EOF while looking for matching `\"'\n",
        );
        assert_eq!(err.line, Some(3));
        assert_eq!(err.snippet.as_deref(), Some("ls \"a"));
    }

    #[test]
    fn uses_echoed_source_for_token_errors() {
        let err = parse_bash_syntax_error(
            "echo a\nls !(x)",
            "bash: -c: line 2: syntax error near unexpected token `('\nbash: -c: line 2: `ls !(x)'\n",
        );
        assert_eq!(err.line, Some(2));
        assert_eq!(err.unmatched_quote, None);
        assert_eq!(err.detail, "syntax error near unexpected token `('");
        assert_eq!(err.snippet.as_deref(), Some("ls !(x)"));
    }

    #[test]
    fn truncates_long_snippets_on_char_boundary() {
        let long = "가".repeat(SNIPPET_MAX_CHARS + 10);
        let err = parse_bash_syntax_error(
            &long,
            "bash: -c: line 1: unexpected EOF while looking for matching `''\n",
        );
        let snippet = err.snippet.expect("snippet");
        assert!(snippet.ends_with("..."));
        assert_eq!(snippet.chars().count(), SNIPPET_MAX_CHARS + 3);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn bash_check_rejects_broken_quotes_and_accepts_valid_commands() {
        if !crate::utils::platform::command_exists("bash") {
            return;
        }
        let err = check_bash_syntax(ISSUE_EXAMPLE)
            .await
            .expect("broken quote must be rejected");
        assert_eq!(err.unmatched_quote, Some('"'));
        assert_eq!(err.line, Some(1));

        let fixed = r#"for pr in 1 2; do echo "x ($(echo $pr | wc -l) lines)"; done"#;
        assert_eq!(check_bash_syntax(fixed).await, None);
        // Heredoc without terminator is only a warning; must not block.
        assert_eq!(check_bash_syntax("cat <<EOF\nhi").await, None);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn bash_check_does_not_execute_the_command() {
        if !crate::utils::platform::command_exists("bash") {
            return;
        }
        let dir = tempfile::tempdir().expect("tempdir");
        let marker = dir.path().join("marker");
        let command = format!("touch '{}'", marker.display());
        assert_eq!(check_bash_syntax(&command).await, None);
        assert!(!marker.exists());
    }
}
