use std::collections::HashMap;
use std::path::{Path, PathBuf};

use glob::glob;

use crate::mcp::builtin::utils::matches_sensitive_path_policy;

use super::super::validation;

const COMMAND_PREVIEW_LIMIT: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellPolicyAction {
    Allow,
    RequireApproval,
    RequireHardApproval,
    Block,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellPolicyDecision {
    pub action: ShellPolicyAction,
    pub reason: String,
    pub description: String,
    pub input_preview: String,
}

pub struct ShellPolicyContext<'a> {
    pub tool_name: &'a str,
    pub command: &'a str,
    pub workspace_dir: Option<&'a Path>,
    pub current_dir: Option<&'a Path>,
    pub environment: Option<&'a HashMap<String, String>>,
    pub force_approval: bool,
}

pub fn is_shell_tool_name(tool_name: &str) -> bool {
    let normalized = tool_name.rsplit("__").next().unwrap_or(tool_name);
    matches!(
        normalized,
        "runShell"
            | "runInPersistentShell"
            | "spawnProcess"
            | "runPowerShell"
            | "runInPersistentPowerShell"
            | "runCmd"
            | "runInPersistentCmd"
    )
}

pub fn evaluate_shell_policy(context: ShellPolicyContext<'_>) -> ShellPolicyDecision {
    let preview = build_command_preview(context.command);

    if let Some(reason) = detect_dynamic_shell_evaluation(context.command) {
        return ShellPolicyDecision {
            action: ShellPolicyAction::RequireHardApproval,
            description: format!(
                "Claude requested hard approval to run tool {}. Risk summary: {}.",
                context.tool_name, reason
            ),
            input_preview: preview,
            reason,
        };
    }

    let path_inspection = inspect_command_paths(
        context.command,
        context.workspace_dir,
        context.current_dir,
        context.environment,
    );

    if let Some(reason) = path_inspection.blocked_reason {
        return ShellPolicyDecision {
            action: ShellPolicyAction::Block,
            description: format!(
                "Blocked {} because it targets a protected location",
                context.tool_name
            ),
            input_preview: preview,
            reason,
        };
    }

    if let Some(reason) = detect_destructive_command(context.command) {
        return ShellPolicyDecision {
            action: ShellPolicyAction::Block,
            description: format!(
                "Blocked {} because it matches a hard shell safety rule",
                context.tool_name
            ),
            input_preview: preview,
            reason,
        };
    }

    if !path_inspection.hard_approval_reasons.is_empty() {
        let reason = path_inspection.hard_approval_reasons.join("; ");
        return ShellPolicyDecision {
            action: ShellPolicyAction::RequireHardApproval,
            description: format!(
                "Claude requested hard approval to run tool {}. Risk summary: {}.",
                context.tool_name, reason
            ),
            input_preview: preview,
            reason,
        };
    }

    let mut reasons = path_inspection.approval_reasons;
    if context.force_approval {
        reasons.push("shell command execution requires approval by policy".to_string());
    }
    if validation::detect_privilege_escalation(context.command) {
        reasons.push("command requests privilege escalation".to_string());
    }
    if validation::is_likely_interactive_command(context.command) {
        reasons.push("command may wait for interactive input".to_string());
    }

    if reasons.is_empty() {
        ShellPolicyDecision {
            action: ShellPolicyAction::Allow,
            description: format!("Allow {}", context.tool_name),
            input_preview: preview,
            reason: "shell command allowed".to_string(),
        }
    } else {
        let reason = reasons.join("; ");
        ShellPolicyDecision {
            action: ShellPolicyAction::RequireApproval,
            description: format!(
                "Claude requested approval to run tool {}. Risk summary: {}.",
                context.tool_name, reason
            ),
            input_preview: preview,
            reason,
        }
    }
}

struct ShellPathInspection {
    blocked_reason: Option<String>,
    hard_approval_reasons: Vec<String>,
    approval_reasons: Vec<String>,
}

fn inspect_command_paths(
    command: &str,
    workspace_dir: Option<&Path>,
    current_dir: Option<&Path>,
    environment: Option<&HashMap<String, String>>,
) -> ShellPathInspection {
    let mut cwd = current_dir
        .map(Path::to_path_buf)
        .or_else(|| workspace_dir.map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."));
    let mut shell_variables = build_initial_shell_variables(environment);
    let mut sensitive_path_cache = HashMap::new();
    let mut hard_approval_reasons = Vec::new();
    let approval_reasons = Vec::new();

    if candidate_targets_sensitive_path(&cwd, &mut sensitive_path_cache) {
        return ShellPathInspection {
            blocked_reason: Some(format!(
                "current shell directory '{}' is protected",
                cwd.display()
            )),
            hard_approval_reasons,
            approval_reasons,
        };
    }

    for segment in split_shell_segments(command) {
        let words = split_shell_words(&segment);
        if words.is_empty() {
            continue;
        }

        let executable_words = apply_shell_variable_assignments(&words, &mut shell_variables);
        if executable_words.is_empty() {
            continue;
        }

        if let Some(reason) =
            detect_unresolved_path_variable_usage(&executable_words, &shell_variables, &cwd)
        {
            hard_approval_reasons.push(reason);
        }

        if let Some(next_cwd) = resolve_cd_target(&executable_words, &cwd, &shell_variables) {
            cwd = next_cwd;
            if candidate_targets_sensitive_path(&cwd, &mut sensitive_path_cache) {
                return ShellPathInspection {
                    blocked_reason: Some(format!(
                        "command changes into protected directory '{}'",
                        cwd.display()
                    )),
                    hard_approval_reasons,
                    approval_reasons,
                };
            }
            continue;
        }

        for word in executable_words {
            if let Some(reason) = detect_sensitive_glob_match(
                &word,
                &cwd,
                &shell_variables,
                &mut sensitive_path_cache,
            ) {
                return ShellPathInspection {
                    blocked_reason: Some(reason),
                    hard_approval_reasons,
                    approval_reasons,
                };
            }

            if token_contains_glob(&word) {
                hard_approval_reasons.push(format!(
                    "command uses dynamic glob expansion in path-like token '{}'",
                    word
                ));
            }

            if let Some(candidate_path) = resolve_candidate_path(&word, &cwd, &shell_variables) {
                if candidate_targets_sensitive_path(&candidate_path, &mut sensitive_path_cache) {
                    return ShellPathInspection {
                        blocked_reason: Some(format!(
                            "command references protected path '{}' (resolved to '{}')",
                            word,
                            candidate_path.display()
                        )),
                        hard_approval_reasons,
                        approval_reasons,
                    };
                }
            }
        }
    }

    ShellPathInspection {
        blocked_reason: None,
        hard_approval_reasons,
        approval_reasons,
    }
}

fn detect_destructive_command(command: &str) -> Option<String> {
    let lower = command.to_lowercase();
    let normalized = lower.split_whitespace().collect::<Vec<_>>().join(" ");
    let hard_block_patterns = [
        (
            "rm -rf /",
            "command attempts to recursively remove the filesystem root",
        ),
        (
            "rm -fr /",
            "command attempts to recursively remove the filesystem root",
        ),
        (
            "rm -rf ~",
            "command attempts to recursively remove the user home directory",
        ),
        (
            "rm -fr ~",
            "command attempts to recursively remove the user home directory",
        ),
        (
            "rm -rf $home",
            "command attempts to recursively remove the user home directory",
        ),
        (
            "rm -rf ${home}",
            "command attempts to recursively remove the user home directory",
        ),
        (
            "remove-item -recurse -force c:\\",
            "command attempts to recursively remove a Windows drive root",
        ),
        (
            "del /s /q c:\\",
            "command attempts to recursively delete a Windows drive root",
        ),
        (
            "rd /s /q c:\\",
            "command attempts to recursively delete a Windows drive root",
        ),
        ("format c:", "command attempts to format a Windows drive"),
    ];

    hard_block_patterns
        .iter()
        .find_map(|(pattern, reason)| normalized.contains(pattern).then(|| (*reason).to_string()))
}

fn detect_dynamic_shell_evaluation(command: &str) -> Option<String> {
    if command.contains("$(")
        || command.contains('`')
        || command.contains("<(")
        || command.contains(">(")
    {
        return Some(
            "command uses shell substitution syntax that cannot be safely policy-checked"
                .to_string(),
        );
    }

    let lowered_words = split_shell_words(command)
        .into_iter()
        .map(|word| word.to_lowercase())
        .collect::<Vec<_>>();
    if lowered_words
        .iter()
        .any(|word| matches!(word.as_str(), "eval" | "source"))
    {
        return Some(
            "command uses dynamic shell evaluation that cannot be safely policy-checked"
                .to_string(),
        );
    }

    None
}

fn build_command_preview(command: &str) -> String {
    let trimmed = command.trim();
    let chars = trimmed.chars().collect::<Vec<_>>();
    if chars.len() <= COMMAND_PREVIEW_LIMIT {
        return trimmed.to_string();
    }

    let head_len = 140usize;
    let tail_len = 40usize;
    let head = chars.iter().take(head_len).collect::<String>();
    let tail = chars
        .iter()
        .rev()
        .take(tail_len)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();

    format!("{head} … {tail}")
}

fn split_shell_segments(command: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut current = String::new();
    let mut chars = command.chars().peekable();
    let mut in_single = false;
    let mut in_double = false;

    while let Some(ch) = chars.next() {
        match ch {
            '\'' if !in_double => {
                in_single = !in_single;
                current.push(ch);
            }
            '"' if !in_single => {
                in_double = !in_double;
                current.push(ch);
            }
            '&' if !in_single && !in_double && chars.peek() == Some(&'&') => {
                chars.next();
                push_segment(&mut segments, &mut current);
            }
            '|' if !in_single && !in_double => {
                if chars.peek() == Some(&'|') {
                    chars.next();
                }
                push_segment(&mut segments, &mut current);
            }
            ';' | '\n' if !in_single && !in_double => {
                push_segment(&mut segments, &mut current);
            }
            _ => current.push(ch),
        }
    }

    push_segment(&mut segments, &mut current);
    segments
}

fn split_shell_words(segment: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut in_single = false;
    let mut in_double = false;
    let mut chars = segment.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '\'' if !in_double => {
                in_single = !in_single;
            }
            '"' if !in_single => {
                in_double = !in_double;
            }
            '\\' if !in_single => {
                if let Some(next) = chars.next() {
                    current.push(next);
                }
            }
            '<' | '>' if !in_single && !in_double => {
                if !current.is_empty() {
                    words.push(current.clone());
                    current.clear();
                }

                let mut operator = String::from(ch);
                if chars.peek() == Some(&ch) {
                    operator.push(chars.next().expect("peeked redirection character"));
                }
                words.push(operator);
            }
            ch if ch.is_whitespace() && !in_single && !in_double => {
                if !current.is_empty() {
                    words.push(current.clone());
                    current.clear();
                }
            }
            _ => current.push(ch),
        }
    }

    if !current.is_empty() {
        words.push(current);
    }

    words
}

fn resolve_cd_target(
    words: &[String],
    cwd: &Path,
    shell_variables: &HashMap<String, String>,
) -> Option<PathBuf> {
    let command = words.first()?.to_lowercase();
    if command == "cd" || command == "pushd" {
        return words
            .get(1)
            .and_then(|target| resolve_candidate_path(target, cwd, shell_variables));
    }

    if command == "set-location" {
        if let Some(target) = words.get(1) {
            if target.starts_with('-') {
                return words
                    .get(2)
                    .and_then(|path| resolve_candidate_path(path, cwd, shell_variables));
            }
            return resolve_candidate_path(target, cwd, shell_variables);
        }
    }

    None
}

fn resolve_candidate_path(
    token: &str,
    cwd: &Path,
    shell_variables: &HashMap<String, String>,
) -> Option<PathBuf> {
    let expanded = expand_path_token_string(token, cwd, shell_variables)?;
    if token_contains_glob(token) && !expanded.exists() {
        return None;
    }
    Some(expanded)
}

fn expand_path_token_string(
    token: &str,
    cwd: &Path,
    shell_variables: &HashMap<String, String>,
) -> Option<PathBuf> {
    let trimmed = token.trim_matches(|ch| matches!(ch, '"' | '\'' | ',' | ';' | '(' | ')'));
    if trimmed.is_empty() || trimmed.starts_with('-') || trimmed.contains("://") {
        return None;
    }

    if let Some(path) = expand_home_token(trimmed) {
        return Some(path);
    }

    if let Some(expanded) = expand_shell_variable_token(trimmed, shell_variables) {
        return expand_path_token_string(&expanded, cwd, shell_variables);
    }

    let path = Path::new(trimmed);
    if path.is_absolute() {
        return Some(path.to_path_buf());
    }

    if trimmed.len() >= 3
        && trimmed.as_bytes()[1] == b':'
        && matches!(trimmed.as_bytes()[2], b'\\' | b'/')
    {
        return Some(PathBuf::from(trimmed));
    }

    if looks_like_relative_path(trimmed) || token_contains_glob(trimmed) {
        return Some(cwd.join(trimmed));
    }

    None
}

fn build_initial_shell_variables(
    environment: Option<&HashMap<String, String>>,
) -> HashMap<String, String> {
    let mut variables = HashMap::new();

    if let Some(environment) = environment {
        for (key, value) in environment {
            variables.insert(key.clone(), value.clone());
        }
    }

    if let Ok(home) = std::env::var("HOME") {
        variables.entry("HOME".to_string()).or_insert(home);
    }
    if let Ok(user_profile) = std::env::var("USERPROFILE") {
        variables
            .entry("USERPROFILE".to_string())
            .or_insert(user_profile);
    }
    if let Ok(home_path) = std::env::var("HOMEPATH") {
        variables.entry("HOMEPATH".to_string()).or_insert(home_path);
    }

    variables
}

fn apply_shell_variable_assignments(
    words: &[String],
    shell_variables: &mut HashMap<String, String>,
) -> Vec<String> {
    let mut executable_words = Vec::new();
    let mut index = 0usize;

    while index < words.len() {
        let word = &words[index];
        let lower = word.to_lowercase();

        if lower == "export" || lower == "setenv" {
            if let Some(assignment) = words.get(index + 1) {
                capture_shell_assignment(assignment, shell_variables);
                index += 2;
                continue;
            }
        }

        if lower == "set" {
            if let Some(assignment) = words.get(index + 1) {
                if capture_shell_assignment(assignment, shell_variables) {
                    index += 2;
                    continue;
                }
            }
        }

        if executable_words.is_empty() && capture_shell_assignment(word, shell_variables) {
            index += 1;
            continue;
        }

        executable_words.push(word.clone());
        index += 1;
    }

    executable_words
}

fn capture_shell_assignment(token: &str, shell_variables: &mut HashMap<String, String>) -> bool {
    let Some((name, value)) = token.split_once('=') else {
        return false;
    };

    if !is_valid_shell_variable_name(name) {
        return false;
    }

    let normalized_value = value.trim_matches(|ch| matches!(ch, '"' | '\''));
    shell_variables.insert(name.to_string(), normalized_value.to_string());
    true
}

fn is_valid_shell_variable_name(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(ch) if ch == '_' || ch.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn detect_unresolved_path_variable_usage(
    words: &[String],
    shell_variables: &HashMap<String, String>,
    cwd: &Path,
) -> Option<String> {
    let command = words.first().map(|word| word.to_lowercase());

    for (index, word) in words.iter().enumerate() {
        let is_cd_target =
            matches!(command.as_deref(), Some("cd" | "pushd" | "set-location")) && index <= 2;
        if !token_uses_unresolved_path_variable(word, shell_variables) {
            continue;
        }

        if is_cd_target || looks_like_relative_path(word) {
            return Some(format!(
                "command uses unresolved shell variable in path-like token '{}'",
                word
            ));
        }

        let trimmed = word.trim_matches(|ch| matches!(ch, '"' | '\'' | ',' | ';' | '(' | ')'));
        if resolve_candidate_path(trimmed, cwd, shell_variables).is_none()
            && (trimmed.contains('/') || trimmed.contains('\\'))
        {
            return Some(format!(
                "command uses unresolved shell variable in path-like token '{}'",
                word
            ));
        }
    }

    None
}

fn token_uses_unresolved_path_variable(
    token: &str,
    shell_variables: &HashMap<String, String>,
) -> bool {
    let trimmed = token.trim_matches(|ch| matches!(ch, '"' | '\'' | ',' | ';' | '(' | ')'));

    if let Some(var_name) = trimmed
        .strip_prefix("${")
        .and_then(|rest| rest.split('}').next())
    {
        return !shell_variables.contains_key(var_name);
    }

    if let Some(var_name) = trimmed
        .strip_prefix('$')
        .and_then(extract_shell_variable_name)
    {
        return !shell_variables.contains_key(var_name);
    }

    if let Some(var_name) = extract_windows_variable_name(trimmed) {
        return !shell_variables.contains_key(var_name);
    }

    false
}

fn detect_sensitive_glob_match(
    token: &str,
    cwd: &Path,
    shell_variables: &HashMap<String, String>,
    sensitive_path_cache: &mut HashMap<PathBuf, bool>,
) -> Option<String> {
    if !token_contains_glob(token) {
        return None;
    }

    let expanded = expand_path_token_string(token, cwd, shell_variables)?;
    let pattern = expanded.to_string_lossy().to_string();
    let matches = glob(&pattern).ok()?;

    for matched_path in matches.flatten() {
        if candidate_targets_sensitive_path(&matched_path, sensitive_path_cache) {
            return Some(format!(
                "command glob '{}' resolves to protected path '{}'",
                token,
                matched_path.display()
            ));
        }
    }

    None
}

fn expand_shell_variable_token(
    token: &str,
    shell_variables: &HashMap<String, String>,
) -> Option<String> {
    if let Some(rest) = token.strip_prefix("${") {
        let (var_name, suffix) = rest.split_once('}')?;
        let value = shell_variables.get(var_name)?;
        return Some(format!("{value}{suffix}"));
    }

    if let Some(rest) = token.strip_prefix('$') {
        let var_name = extract_shell_variable_name(rest)?;
        let suffix = &rest[var_name.len()..];
        let value = shell_variables.get(var_name)?;
        return Some(format!("{value}{suffix}"));
    }

    if let Some((var_name, suffix)) = extract_windows_variable_parts(token) {
        let value = shell_variables.get(var_name)?;
        return Some(format!("{value}{suffix}"));
    }

    None
}

fn extract_shell_variable_name(value: &str) -> Option<&str> {
    let end = value
        .char_indices()
        .take_while(|(_, ch)| *ch == '_' || ch.is_ascii_alphanumeric())
        .last()
        .map(|(index, ch)| index + ch.len_utf8())?;
    value.get(..end)
}

fn extract_windows_variable_name(value: &str) -> Option<&str> {
    extract_windows_variable_parts(value).map(|(name, _)| name)
}

fn extract_windows_variable_parts(value: &str) -> Option<(&str, &str)> {
    let remainder = value.strip_prefix('%')?;
    let closing = remainder.find('%')?;
    let (name, suffix_with_percent) = remainder.split_at(closing);
    let suffix = suffix_with_percent.strip_prefix('%')?;
    Some((name, suffix))
}

fn candidate_targets_sensitive_path(
    candidate_path: &Path,
    sensitive_path_cache: &mut HashMap<PathBuf, bool>,
) -> bool {
    let cache_key = candidate_path.to_path_buf();
    if let Some(cached) = sensitive_path_cache.get(&cache_key) {
        return *cached;
    }

    if matches_sensitive_path_policy(candidate_path) {
        sensitive_path_cache.insert(cache_key, true);
        return true;
    }

    if let Ok(canonical_path) = candidate_path.canonicalize() {
        let is_sensitive = matches_sensitive_path_policy(&canonical_path);
        sensitive_path_cache.insert(canonical_path, is_sensitive);
        sensitive_path_cache.insert(cache_key, is_sensitive);
        return is_sensitive;
    }

    let mut current = Some(candidate_path);
    while let Some(path) = current {
        if let Some(cached) = sensitive_path_cache.get(path).copied() {
            sensitive_path_cache.insert(cache_key.clone(), cached);
            return cached;
        }
        if let Ok(canonical_path) = path.canonicalize() {
            let is_sensitive = matches_sensitive_path_policy(&canonical_path);
            sensitive_path_cache.insert(path.to_path_buf(), is_sensitive);
            sensitive_path_cache.insert(canonical_path, is_sensitive);
            sensitive_path_cache.insert(cache_key.clone(), is_sensitive);
            return is_sensitive;
        }
        current = path.parent();
    }

    sensitive_path_cache.insert(cache_key, false);
    false
}

fn token_contains_glob(token: &str) -> bool {
    token.contains('*') || token.contains('?') || token.contains('[')
}

fn expand_home_token(token: &str) -> Option<PathBuf> {
    let home = dirs::home_dir()?;

    if token == "~" {
        return Some(home);
    }
    if let Some(rest) = token.strip_prefix("~/") {
        return Some(home.join(rest));
    }
    if let Some(rest) = token.strip_prefix("$HOME/") {
        return Some(home.join(rest));
    }
    if let Some(rest) = token.strip_prefix("${HOME}/") {
        return Some(home.join(rest));
    }

    if let Some(rest) = strip_prefix_ignore_ascii_case(token, "%userprofile%\\") {
        return Some(home.join(rest));
    }
    if let Some(rest) = strip_prefix_ignore_ascii_case(token, "%homepath%\\") {
        return Some(home.join(rest));
    }

    None
}

fn looks_like_relative_path(token: &str) -> bool {
    token.starts_with("./")
        || token.starts_with("../")
        || token.starts_with(".\\")
        || token.starts_with("..\\")
        || token.starts_with('.')
        || token.contains('/')
        || token.contains('\\')
}

fn push_segment(segments: &mut Vec<String>, current: &mut String) {
    let trimmed = current.trim();
    if !trimmed.is_empty() {
        segments.push(trimmed.to_string());
    }
    current.clear();
}

fn strip_prefix_ignore_ascii_case<'a>(value: &'a str, prefix: &str) -> Option<&'a str> {
    value
        .get(..prefix.len())
        .filter(|candidate| candidate.eq_ignore_ascii_case(prefix))
        .and_then(|_| value.get(prefix.len()..))
}
