//! Matcher semantics plus the event labels and working directory a hook
//! run uses.

use crate::services::hooks::envelope::SessionStartSource;
use crate::services::hooks::schema::HookEvent;
use crate::session::{get_session_manager, resolve_session_workspace_dir};
use regex::Regex;
use std::path::PathBuf;

pub fn matcher_matches(matcher: Option<&str>, value: &str) -> bool {
    let Some(pattern) = matcher.map(str::trim).filter(|s| !s.is_empty()) else {
        return true;
    };
    if pattern == "*" {
        return true;
    }
    let is_exact_set = pattern
        .chars()
        .all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | ' ' | ',' | '|'));
    if is_exact_set {
        return pattern
            .split(['|', ','])
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .any(|token| token == value);
    }
    match Regex::new(pattern) {
        Ok(re) => re.is_match(value),
        Err(e) => {
            log::warn!("invalid hook matcher regex {:?}: {e}", pattern);
            false
        }
    }
}

pub(super) fn session_start_source_label(source: SessionStartSource) -> &'static str {
    match source {
        SessionStartSource::Startup => "startup",
        SessionStartSource::Resume => "resume",
        SessionStartSource::Clear => "clear",
        SessionStartSource::Compact => "compact",
    }
}

/// Claude Code tool names that may appear in an `if` rule, mapped to the
/// LibrAgent tool names dispatch actually passes (#3003 G1.4). Without this,
/// a rule copied from a Claude Code config (`Bash(git *)`) never matches a
/// real `workspace__runShell` call. Shell tools are not listed here — they
/// are recognized through [`is_shell_tool`] (#3003 G2.2).
fn claude_rule_tool_aliases(rule_tool: &str) -> &'static [&'static str] {
    match rule_tool {
        "Read" => &["workspace__readFile"],
        "Write" => &["workspace__writeFile"],
        "Edit" | "MultiEdit" | "NotebookEdit" => {
            &["workspace__strReplace", "workspace__editObject"]
        }
        "Glob" => &["workspace__globFiles"],
        "Grep" => &["workspace__grepFiles"],
        "WebFetch" => &["browser__fetchUrl"],
        _ => &[],
    }
}

/// True for every LibrAgent shell tool (Unix and Windows: bash/persistent/
/// spawn, PowerShell, cmd). Delegates to the shell policy's own predicate so
/// there is exactly one list of shell tools (#3003 G2.2) — a rule written as
/// `Bash(...)` or `PowerShell(...)` must guard `workspace__runPowerShell`
/// too, not only the Unix tools.
fn is_shell_tool(tool: &str) -> bool {
    crate::mcp::builtin::workspace::code_execution::shell::policy::is_shell_tool_name(tool)
}

fn rule_tool_matches(rule_tool: &str, tool: &str) -> bool {
    if matches!(rule_tool, "Bash" | "PowerShell") && is_shell_tool(tool) {
        return true;
    }
    claude_rule_tool_aliases(rule_tool).contains(&tool) || matcher_matches(Some(rule_tool), tool)
}

/// Evaluate a Claude Code handler `if` filter (permission-rule syntax such as
/// `Bash(git *)`, `Edit(*.rs)`, or `Read`) (#3003). Per the Hooks reference,
/// `if` is only evaluated on tool events: on any other event a handler with
/// `if` set does not run.
pub fn if_condition_matches(
    condition: Option<&str>,
    event: HookEvent,
    tool_name: Option<&str>,
    tool_input: Option<&serde_json::Value>,
) -> bool {
    let Some(rule) = condition.map(str::trim).filter(|s| !s.is_empty()) else {
        return true;
    };
    if !matches!(
        event,
        HookEvent::PreToolUse
            | HookEvent::PostToolUse
            | HookEvent::PostToolUseFailure
            | HookEvent::PermissionRequest
            | HookEvent::PermissionDenied
    ) {
        return false;
    }
    let Some(tool) = tool_name else {
        // No tool name to evaluate the rule against; the `if` filter is
        // best-effort, so fail toward running the guard (#3003 G1.2).
        return true;
    };
    if let (Some(open), true) = (rule.find('('), rule.ends_with(')')) {
        let rule_tool = rule[..open].trim();
        let arg_pattern = rule[open + 1..rule.len() - 1].trim();
        if !rule_tool_matches(rule_tool, tool) {
            return false;
        }
        if arg_pattern.is_empty() || arg_pattern == "*" {
            return true;
        }
        let Some(arg_text) = extract_tool_input_arg(tool, tool_input) else {
            // Argument unavailable to check → run the guard rather than skip it.
            return true;
        };
        return if is_shell_tool(tool) || matches!(rule_tool, "Bash" | "PowerShell") {
            bash_pattern_matches(arg_pattern, &arg_text)
        } else {
            wildcard_matches(arg_pattern, &arg_text)
        };
    }
    rule_tool_matches(rule, tool)
}

/// Claude Code's documented `Bash(...)` matching (#3003 G1.2): each
/// subcommand is checked on its own, plain leading `VAR=value` assignments are
/// stripped, `$()` and backtick contents are checked, and the hook runs
/// anyway when the command cannot be split, when an assignment value is not a
/// plain token, or when the pattern specifies more than the command name while
/// the command contains an expansion.
fn bash_pattern_matches(pattern: &str, command: &str) -> bool {
    let Some(subcommands) = shell_subcommands(command) else {
        return true;
    };
    // "Patterns that specify more than the command name run the hook anyway
    // on $(), backticks, or $VAR" — their contents cannot be checked without
    // executing the shell.
    let names_more_than_command = pattern
        .split_whitespace()
        .filter(|token| *token != "*")
        .count()
        > 1;
    if names_more_than_command
        && (command.contains("$(") || command.contains('`') || has_dollar_var(command))
    {
        return true;
    }
    for subcommand in subcommands {
        // A leading assignment with an unmodeled (quoted) value makes the
        // command word unidentifiable, so the guard runs (#3003 G2.1).
        let Some(subcommand) = strip_leading_assignments(&subcommand) else {
            return true;
        };
        // Inverted rule (#3003 G2.1 round 3): a segment is judged only when
        // its command word — the first token after leading assignments — is
        // a plain unquoted word that is not a shell reserved word. For
        // anything else (`"rm" -rf x`, `'rm' -rf x`, `r''m -rf x`,
        // `! rm -rf x`, `time rm -rf x`, `>log rm -rf x`, …) the command
        // word cannot be identified, so the whole command is unjudgeable and
        // the guard hook runs instead of the pattern silently missing.
        let Some(word) = subcommand.split_whitespace().next() else {
            return true;
        };
        if !is_plain_command_word(word) || is_reserved_command_word(word) {
            return true;
        }
        if wildcard_matches(pattern.trim(), subcommand) {
            return true;
        }
    }
    false
}

/// Characters a plain, unquoted command word may consist of (#3003 G2.1).
/// Any other character — quotes, `$`, backtick, `>`, `(`, `!`… — means the
/// segment's command word is not a word this matcher can identify.
fn is_plain_command_word(word: &str) -> bool {
    !word.is_empty()
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '/'))
}

/// Shell reserved words: a segment starting with one is control flow the
/// segment splitter does not model (#3003 G2.1).
fn is_reserved_command_word(word: &str) -> bool {
    const RESERVED: [&str; 15] = [
        "if", "then", "elif", "else", "fi", "for", "while", "until", "do", "done", "case", "esac",
        "function", "select", "time",
    ];
    RESERVED.contains(&word)
}

fn has_dollar_var(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.iter().enumerate().any(|(i, b)| {
        *b == b'$'
            && bytes
                .get(i + 1)
                .is_some_and(|n| n.is_ascii_alphabetic() || *n == b'_')
    })
}

/// Split a shell command into the subcommands a `Bash(...)` rule is matched
/// against: top-level `&&`, `||`, `;`, `|`, and newline segments, plus the
/// contents of `$()` and backticks.
///
/// Anything the parser does not fully model — unbalanced quoting, backslash
/// escapes, bare grouping (`(`, `)`, `{`, `}`), the background operator `&`,
/// an unquoted `#` that starts a word (a comment) — returns `None`, and the
/// caller additionally refuses to judge any segment
/// whose command word is not a plain unquoted word
/// ([`bash_pattern_matches`], #3003 G2.1): every unrecognized construct runs
/// the guard hook instead of silently skipping it.
fn shell_subcommands(command: &str) -> Option<Vec<String>> {
    let mut parts: Vec<String> = Vec::new();
    shell_subcommands_into(command, &mut parts)?;
    Some(parts)
}

fn shell_subcommands_into(command: &str, parts: &mut Vec<String>) -> Option<()> {
    let mut current = String::new();
    let mut expansions: Vec<String> = Vec::new();
    let mut in_single = false;
    let mut in_double = false;
    let mut chars = command.chars().peekable();
    while let Some(c) = chars.next() {
        if in_single {
            current.push(c);
            if c == '\'' {
                in_single = false;
            }
            continue;
        }
        match c {
            // Inside double quotes a single quote is an ordinary character,
            // not the start of a single-quoted span (#3003 G2.1:
            // `echo "it's" && rm -rf x` must split on the real `&&`).
            '\'' if in_double => current.push(c),
            '\'' => {
                in_single = true;
                current.push(c);
            }
            '"' => {
                in_double = !in_double;
                current.push(c);
            }
            // Backslash escaping is not modeled; bail rather than let an
            // escaped quote hide a segment from the guard (#3003 G2.1).
            '\\' => return None,
            // Bare grouping is not modeled; bail rather than leave a grouped
            // `rm` inside a single unmatchable part (#3003 G2.1).
            '(' | ')' | '{' | '}' if !in_double => return None,
            '`' => {
                let mut inner = String::new();
                let mut closed = false;
                for nc in chars.by_ref() {
                    if nc == '`' {
                        closed = true;
                        break;
                    }
                    inner.push(nc);
                }
                if !closed {
                    return None;
                }
                current.push('`');
                current.push_str(&inner);
                current.push('`');
                expansions.push(inner);
            }
            '$' if chars.peek() == Some(&'(') => {
                chars.next();
                current.push_str("$(");
                let mut inner = String::new();
                let mut depth = 1usize;
                let mut inner_single = false;
                let mut inner_double = false;
                let mut closed = false;
                for nc in chars.by_ref() {
                    if inner_single {
                        inner.push(nc);
                        if nc == '\'' {
                            inner_single = false;
                        }
                        continue;
                    }
                    if inner_double {
                        inner.push(nc);
                        if nc == '"' {
                            inner_double = false;
                        }
                        continue;
                    }
                    match nc {
                        '\'' => {
                            inner_single = true;
                            inner.push(nc);
                        }
                        '"' => {
                            inner_double = true;
                            inner.push(nc);
                        }
                        '(' => {
                            depth += 1;
                            inner.push(nc);
                        }
                        ')' => {
                            depth -= 1;
                            if depth == 0 {
                                closed = true;
                                break;
                            }
                            inner.push(nc);
                        }
                        _ => inner.push(nc),
                    }
                }
                if !closed {
                    return None;
                }
                current.push_str(&inner);
                current.push(')');
                expansions.push(inner);
            }
            // A `#` that starts a word begins a comment, which this splitter
            // does not model: two apostrophes in a comment would otherwise
            // open and close a quoted span and swallow the command between
            // them (`true # don't` / `rm -rf build` / `# it's done`), so the
            // guard would silently not run (#3003 G2.1).
            '#' if !in_double && (current.is_empty() || current.ends_with(char::is_whitespace)) => {
                return None;
            }
            // An unquoted `<<` starts a heredoc or here-string, whose body is
            // not shell syntax: a quote inside one heredoc body would pair with
            // a quote in a later body and swallow the command between them
            // (#3003 G3.1). Bail so the guard runs.
            '<' if !in_double && chars.peek() == Some(&'<') => return None,
            '&' if !in_double && chars.peek() == Some(&'&') => {
                chars.next();
                flush_shell_part(&mut current, parts);
            }
            // A lone `&` backgrounds the command, so a later segment such as
            // `rm -rf x` still runs; not modeled — bail (#3003 G2.1).
            '&' if !in_double => return None,
            '|' if !in_double && chars.peek() == Some(&'|') => {
                chars.next();
                flush_shell_part(&mut current, parts);
            }
            '|' | ';' | '\n' if !in_double => flush_shell_part(&mut current, parts),
            _ => current.push(c),
        }
    }
    if in_single || in_double {
        return None;
    }
    flush_shell_part(&mut current, parts);
    for expansion in expansions {
        shell_subcommands_into(&expansion, parts)?;
    }
    Some(())
}

fn flush_shell_part(current: &mut String, parts: &mut Vec<String>) {
    if !current.trim().is_empty() {
        parts.push(current.trim().to_string());
    }
    current.clear();
}

/// Strip leading `VAR=value` assignments before matching, so
/// `FOO=bar git push` matches `git *` (#3003 G1.2).
///
/// Returns `None` when an assignment's value is not a plain token: this split
/// relies on whitespace being the token boundary, and a quoted value breaks
/// that (`MSG="fix the bug" git push` would leave `the` as the command word
/// and silently skip the guard). Rather than grow a quote-aware parser, an
/// unmodeled value makes the command unjudgeable, so the guard runs
/// (#3003 G2.1).
fn strip_leading_assignments(subcommand: &str) -> Option<&str> {
    let mut rest = subcommand;
    loop {
        match rest.split_once(char::is_whitespace) {
            Some((head, tail)) if is_assignment(head) => {
                if !is_plain_assignment_token(head) {
                    return None;
                }
                rest = tail.trim_start();
            }
            _ => return Some(rest),
        }
    }
}

/// An assignment whose value is a plain unquoted token. Quotes, `$`, backtick
/// and backslash are not modeled by the splitter, and any of them can hide a
/// token boundary the matcher would otherwise trust (#3003 G2.1).
fn is_plain_assignment_token(token: &str) -> bool {
    token
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '/' | ':' | '=' | ','))
}

fn is_assignment(token: &str) -> bool {
    let Some(eq) = token.find('=') else {
        return false;
    };
    let name = &token[..eq];
    !name.is_empty()
        && !name.as_bytes()[0].is_ascii_digit()
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'.')
}

/// Pick the argument the rule should match against. Shell tools run on
/// `command`/`script`; file-oriented tools on `path` (#3003 G1.4, S1.1).
fn extract_tool_input_arg(tool: &str, tool_input: Option<&serde_json::Value>) -> Option<String> {
    let input = tool_input?;
    match input {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Object(map) => {
            let keys: &[&str] = if is_shell_tool(tool) {
                &["command", "script"]
            } else {
                &["path", "file_path", "command", "cmd", "url", "pattern"]
            };
            for key in keys {
                if let Some(s) = map.get(*key).and_then(|v| v.as_str()) {
                    return Some(s.to_string());
                }
            }
            if is_shell_tool(tool) {
                return None;
            }
            let joined: Vec<&str> = map.values().filter_map(|v| v.as_str()).collect();
            if !joined.is_empty() {
                Some(joined.join(" "))
            } else {
                Some(input.to_string())
            }
        }
        _ => Some(input.to_string()),
    }
}

fn wildcard_matches(pattern: &str, text: &str) -> bool {
    let mut regex = String::from("^");
    for ch in pattern.chars() {
        if ch == '*' {
            regex.push_str(".*");
        } else {
            regex.push_str(&regex::escape(&ch.to_string()));
        }
    }
    regex.push('$');
    // (?s): a multi-line command must still be matched across newlines (#3003 G1.2).
    Regex::new(&format!("(?s){regex}")).is_ok_and(|re| re.is_match(text))
}

pub(in crate::services::hooks) async fn resolve_session_cwd(
    session_id: &str,
    override_cwd: Option<PathBuf>,
) -> Result<PathBuf, String> {
    if let Some(p) = override_cwd {
        return Ok(p);
    }
    let session_manager = get_session_manager()?;
    resolve_session_workspace_dir(session_manager, session_id).await
}
