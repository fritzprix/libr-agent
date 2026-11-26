//! Matcher semantics and `if`-rule matching (#3003): the pure functions the
//! dispatcher calls, tested without a hook subprocess.

use super::*;
use crate::services::hooks::execute::matcher::if_condition_matches;

#[test]
fn matcher_or_patterns() {
    assert!(matcher_matches(Some("startup|clear"), "startup"));
    assert!(!matcher_matches(Some("startup|clear"), "resume"));
    assert!(matcher_matches(None, "anything"));
    assert!(matcher_matches(Some(""), "anything"));
    assert!(matcher_matches(Some("*"), "anything"));
}

#[test]
fn matcher_exact_lists_and_regex_contract() {
    // Exact-match character set: | and , separated exact strings
    assert!(matcher_matches(Some("Edit|Write"), "Edit"));
    assert!(matcher_matches(Some("Edit|Write"), "Write"));
    assert!(!matcher_matches(Some("Edit|Write"), "EditFile"));
    assert!(matcher_matches(Some("Edit, Write"), "Edit"));
    assert!(matcher_matches(Some("Edit, Write"), "Write"));
    assert!(!matcher_matches(Some("Edit, Write"), "Read"));

    // Regex characters fall back to unanchored regex
    assert!(matcher_matches(Some("^Notebook"), "NotebookEdit"));
    assert!(!matcher_matches(Some("^Notebook"), "MyNotebook"));
}

#[test]
fn if_condition_permission_rule_matching() {
    let git_status = serde_json::json!({ "command": "git status" });
    let npm_test = serde_json::json!({ "command": "npm test" });

    // Dispatch passes real LibrAgent tool names (#3003 G1.4), so the
    // Claude name `Bash` must map onto `workspace__runShell`.
    assert!(if_condition_matches(
        Some("Bash(git *)"),
        HookEvent::PreToolUse,
        Some("workspace__runShell"),
        Some(&git_status),
    ));
    assert!(!if_condition_matches(
        Some("Bash(git *)"),
        HookEvent::PreToolUse,
        Some("workspace__runShell"),
        Some(&npm_test),
    ));
    assert!(!if_condition_matches(
        Some("Bash(git *)"),
        HookEvent::PreToolUse,
        Some("workspace__writeFile"),
        Some(&git_status),
    ));
    // Rules may also name the LibrAgent tool directly.
    assert!(if_condition_matches(
        Some("workspace__runShell(git *)"),
        HookEvent::PreToolUse,
        Some("workspace__runShell"),
        Some(&git_status),
    ));
    // A bare tool rule matches through the alias too.
    assert!(if_condition_matches(
        Some("Bash"),
        HookEvent::PreToolUse,
        Some("workspace__runShell"),
        None,
    ));
    // Omitted `if` matches everything
    assert!(if_condition_matches(
        None,
        HookEvent::PreToolUse,
        Some("workspace__writeFile"),
        Some(&npm_test),
    ));
}

#[test]
fn if_condition_bash_subcommands_expansions_and_failures() {
    let run = |rule: &str, command: &str| {
        if_condition_matches(
            Some(rule),
            HookEvent::PreToolUse,
            Some("workspace__runShell"),
            Some(&serde_json::json!({ "command": command })),
        )
    };
    // Documented table: each subcommand is checked, leading assignments
    // are stripped.
    assert!(run("Bash(git *)", "npm test && git push"));
    assert!(run("Bash(git *)", "FOO=bar git push"));
    // `$()` and backtick contents are checked.
    assert!(run("Bash(rm *)", "echo $(rm -rf /tmp/build)"));
    assert!(run("Bash(rm *)", "echo `rm -rf /tmp/build`"));
    assert!(!run("Bash(rm *)", "echo $(date)"));
    // A pattern naming more than the command runs anyway on expansions.
    assert!(run("Bash(git push *)", "echo $(date)"));
    // Multi-line commands: `.*` must cross newlines.
    assert!(run("Bash(git *)", "echo one\ngit push"));
    // Unparseable commands run the guard rather than skip it.
    assert!(run("Bash(git *)", "echo \"unclosed"));
    assert!(run("Bash(git *)", "echo $(unbalanced"));
    // Genuine misses still miss.
    assert!(!run("Bash(git *)", "npm test && cargo build"));
}

/// Every construct the segment splitter does not fully model must run the
/// guard hook instead of silently skipping it (#3003 G2.1) — a skipped
/// PreToolUse guard is a bypass.
#[test]
fn if_condition_bash_unmodeled_constructs_run_the_guard() {
    let run = |rule: &str, command: &str| {
        if_condition_matches(
            Some(rule),
            HookEvent::PreToolUse,
            Some("workspace__runShell"),
            Some(&serde_json::json!({ "command": command })),
        )
    };
    // A lone `&` backgrounds the first segment; `rm` still runs.
    assert!(run("Bash(rm *)", "echo hi & rm -rf x"));
    // Grouping and control flow keep `rm` reachable at runtime.
    assert!(run("Bash(rm *)", "(rm -rf x)"));
    assert!(run("Bash(rm *)", "{ rm -rf x; }"));
    assert!(run("Bash(rm *)", "if true; then rm -rf x; fi"));
    // Backslash escapes must not hide a segment from the guard.
    // (Rust-escaped: the shell command contains literal backslashes.)
    assert!(run("Bash(rm *)", "echo \\\"; rm -rf x; echo \\\""));
    // A `'` inside double quotes is a literal, not a quote span.
    assert!(run(
        "Bash(rm *)",
        "echo \"it's\" && rm -rf x && echo \"it's\""
    ));
    // Round 3 (#3003 G2.1): the command word itself must be a plain
    // unquoted word, or the whole command is unjudgeable and the guard
    // runs. Quoted, reserved-word, negated, or redirection-led command
    // words all fall here.
    assert!(run("Bash(rm *)", "\"rm\" -rf x"));
    assert!(run("Bash(rm *)", "'rm' -rf x"));
    assert!(run("Bash(rm *)", "r''m -rf x"));
    assert!(run("Bash(rm *)", "! rm -rf x"));
    assert!(run("Bash(rm *)", "time rm -rf x"));
    assert!(run("Bash(rm *)", ">log rm -rf x"));
    // Round 2 (#3003 G2.1): an assignment value the splitter cannot read and
    // an unquoted `#` comment both hide the real command word, so the guard
    // runs instead of the rule silently missing.
    assert!(run("Bash(git *)", "MSG=\"fix the bug\" git push"));
    assert!(run("Bash(rm *)", "A='x y z' rm -rf build"));
    assert!(run("Bash(rm *)", "true # don't\nrm -rf build\n# it's done"));
    assert!(run(
        "Bash(rm *)",
        "cat <<A\nsay \"hi\nA\nrm -rf build\ncat <<B\nbye\" now\nB",
    ));
    // Plain assignments stay judgeable, so a real miss still misses.
    assert!(!run("Bash(git *)", "FOO=bar npm test"));
    assert!(run("Bash(git *)", "FOO=bar git push"));
}

/// `Bash(...)`/`PowerShell(...)` rules must guard the Windows shell tools
/// too — the predicate is the shell policy's own list, single source
/// (#3003 G2.2).
#[test]
fn if_condition_bash_rules_guard_powershell_tools() {
    let git_status = serde_json::json!({ "command": "git status" });
    assert!(if_condition_matches(
        Some("PowerShell(git *)"),
        HookEvent::PreToolUse,
        Some("workspace__runPowerShell"),
        Some(&git_status),
    ));
    assert!(if_condition_matches(
        Some("Bash(git *)"),
        HookEvent::PreToolUse,
        Some("workspace__runInPersistentPowerShell"),
        Some(&git_status),
    ));
    assert!(!if_condition_matches(
        Some("PowerShell(git *)"),
        HookEvent::PreToolUse,
        Some("workspace__runPowerShell"),
        Some(&serde_json::json!({ "command": "npm test" })),
    ));
}

#[test]
fn if_condition_on_non_tool_event_does_not_run() {
    // Hooks reference: on events other than tool events, a handler with
    // `if` set does not run (#3003 G1.3).
    assert!(!if_condition_matches(
        Some("Bash(git *)"),
        HookEvent::SessionStart,
        None,
        None,
    ));
    // Without `if`, non-tool events are unaffected.
    assert!(if_condition_matches(
        None,
        HookEvent::SessionStart,
        None,
        None,
    ));
}

#[test]
fn if_condition_matches_file_tools_on_path_argument() {
    assert!(if_condition_matches(
        Some("Edit(*.rs)"),
        HookEvent::PreToolUse,
        Some("workspace__strReplace"),
        Some(&serde_json::json!({
            "path": "src/main.rs",
            "old_string": "a",
            "new_string": "b"
        })),
    ));
    assert!(!if_condition_matches(
        Some("Edit(*.rs)"),
        HookEvent::PreToolUse,
        Some("workspace__strReplace"),
        Some(&serde_json::json!({ "path": "README.md" })),
    ));
}
