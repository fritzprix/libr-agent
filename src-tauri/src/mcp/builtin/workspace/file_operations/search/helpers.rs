use crate::mcp::builtin::error_guidance::{guided_error, ErrorCategory, ToolGroup};
use crate::mcp::builtin::workspace::utils::is_internal_workspace_artifact_path;
use crate::mcp::types::MCPResult;
use serde_json::Value;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use tokio::io::AsyncReadExt;

pub(super) const BINARY_SNIFF_BYTES: usize = 8 * 1024;
pub(super) const MAX_SEARCH_CONTENT_FILE_SIZE: usize = 5 * 1024 * 1024;
/// Cap for a single grep matching line in LLM/UI output.
/// Aligns with `readFile` continuation `size: 200` so agents can reopen the line.
pub(super) const MAX_GREP_MATCH_LINE_CHARS: usize = 200;
pub(super) const SKIPPED_SEARCH_DIR_NAMES: &[&str] = &[
    ".git",
    "node_modules",
    "dist",
    "target",
    ".next",
    "coverage",
];
const MAX_BRACE_GLOB_EXPANSIONS: usize = 64;

pub(super) struct GlobMatcher {
    patterns: Vec<glob::Pattern>,
}

impl GlobMatcher {
    pub(super) fn parse(pattern: &str) -> Result<Self, String> {
        let expanded = expand_brace_globs(pattern)?;
        if expanded.len() > MAX_BRACE_GLOB_EXPANSIONS {
            return Err(format!(
                "filePattern expands to {} globs (max {MAX_BRACE_GLOB_EXPANSIONS})",
                expanded.len()
            ));
        }

        let mut patterns = Vec::with_capacity(expanded.len());
        for item in expanded {
            match glob::Pattern::new(&item) {
                Ok(parsed) => patterns.push(parsed),
                Err(error) => {
                    return Err(format!("Invalid filePattern `{item}`: {error}"));
                }
            }
        }
        Ok(Self { patterns })
    }
}

pub(super) enum SearchEntrySkipReason {
    Gitignored,
    HeavyweightDirectory,
    InternalArtifactDirectory,
}

pub(super) struct ScopedGitignoreMatcher {
    search_root: PathBuf,
    matchers: Vec<(PathBuf, ignore::gitignore::Gitignore)>,
}

impl ScopedGitignoreMatcher {
    fn is_ignored(&self, path: &Path, is_dir: bool) -> bool {
        let scope_target = if is_dir {
            path
        } else {
            path.parent().unwrap_or(&self.search_root)
        };

        let mut scoped_ancestors = Vec::new();
        for ancestor in scope_target.ancestors() {
            if !ancestor.starts_with(&self.search_root) {
                break;
            }
            scoped_ancestors.push(ancestor);
            if ancestor == self.search_root {
                break;
            }
        }
        scoped_ancestors.reverse();

        let mut ignored = false;
        for ancestor in scoped_ancestors {
            if let Some((_, matcher)) = self.matchers.iter().find(|(scope, _)| scope == ancestor) {
                let matched = matcher.matched(path, is_dir);
                if matched.is_ignore() {
                    ignored = true;
                } else if matched.is_whitelist() {
                    ignored = false;
                }
            }
        }

        ignored
    }
}

pub(super) fn matches_glob(matcher: &GlobMatcher, path: &Path, file_name: Option<&str>) -> bool {
    matcher
        .patterns
        .iter()
        .any(|pattern| glob_pattern_matches(pattern, path, file_name))
}

fn glob_pattern_matches(pattern: &glob::Pattern, path: &Path, file_name: Option<&str>) -> bool {
    if let Some(name) = file_name {
        if pattern.matches(name) {
            return true;
        }
    }
    let path_str = path.to_string_lossy();
    if pattern.matches(&path_str) {
        return true;
    }
    #[cfg(target_os = "windows")]
    if path_str.contains('\\') {
        let normalized = path_str.replace('\\', "/");
        if pattern.matches(&normalized) {
            return true;
        }
    }
    false
}

fn expand_brace_globs(pattern: &str) -> Result<Vec<String>, String> {
    let mut pending = vec![pattern.to_string()];
    let mut expanded = Vec::new();

    while let Some(current) = pending.pop() {
        match next_brace_expansion(&current)? {
            None => {
                if !expanded.contains(&current) {
                    expanded.push(current);
                }
            }
            Some(parts) => {
                if expanded.len() + pending.len() + parts.len() > MAX_BRACE_GLOB_EXPANSIONS {
                    return Err(format!(
                        "filePattern expands to more than {MAX_BRACE_GLOB_EXPANSIONS} globs"
                    ));
                }
                pending.extend(parts);
            }
        }
    }

    if expanded.is_empty() {
        expanded.push(pattern.to_string());
    }
    Ok(expanded)
}

fn next_brace_expansion(pattern: &str) -> Result<Option<Vec<String>>, String> {
    let Some((start, end, alternatives)) = find_expandable_brace_group(pattern)? else {
        return Ok(None);
    };
    let prefix = &pattern[..start];
    let suffix = &pattern[end + 1..];
    Ok(Some(
        alternatives
            .into_iter()
            .map(|alternative| format!("{prefix}{alternative}{suffix}"))
            .collect(),
    ))
}

fn find_expandable_brace_group(
    pattern: &str,
) -> Result<Option<(usize, usize, Vec<String>)>, String> {
    let bytes = pattern.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'{' {
            match parse_brace_group(pattern, index)? {
                Some((end, alternatives)) => return Ok(Some((index, end, alternatives))),
                None => index += 1,
            }
        } else {
            index += 1;
        }
    }
    Ok(None)
}

fn parse_brace_group(
    pattern: &str,
    open_index: usize,
) -> Result<Option<(usize, Vec<String>)>, String> {
    let mut depth = 0usize;
    let mut alternative_start = open_index + 1;
    let mut alternatives = Vec::new();
    let mut saw_comma = false;

    for (offset, character) in pattern[open_index..].char_indices() {
        let absolute = open_index + offset;
        match character {
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    alternatives.push(pattern[alternative_start..absolute].to_string());
                    if !saw_comma {
                        return Ok(None);
                    }
                    return Ok(Some((absolute, alternatives)));
                }
            }
            ',' if depth == 1 => {
                saw_comma = true;
                alternatives.push(pattern[alternative_start..absolute].to_string());
                alternative_start = absolute + 1;
            }
            _ => {}
        }
    }

    Err("Invalid filePattern: unmatched `{` in brace glob".to_string())
}

pub(super) fn classify_search_entry_skip(
    workspace_root: &Path,
    entry: &walkdir::DirEntry,
    gitignore: Option<&ScopedGitignoreMatcher>,
) -> Option<SearchEntrySkipReason> {
    if entry.depth() == 0 {
        return None;
    }

    if entry.file_type().is_dir()
        && is_internal_workspace_artifact_path(workspace_root, entry.path())
    {
        return Some(SearchEntrySkipReason::InternalArtifactDirectory);
    }

    if let Some(gitignore_matcher) = gitignore {
        if gitignore_matcher.is_ignored(entry.path(), entry.file_type().is_dir()) {
            return Some(SearchEntrySkipReason::Gitignored);
        }
    }

    if entry.file_type().is_dir() {
        return entry
            .file_name()
            .to_str()
            .filter(|name| SKIPPED_SEARCH_DIR_NAMES.contains(name))
            .map(|_| SearchEntrySkipReason::HeavyweightDirectory);
    }

    None
}

pub(super) fn effective_search_content_file_size_limit() -> usize {
    crate::config::max_file_size().min(MAX_SEARCH_CONTENT_FILE_SIZE)
}

pub(super) async fn is_probably_binary_file(path: &Path) -> bool {
    let mut file = match tokio::fs::File::open(path).await {
        Ok(file) => file,
        Err(_) => return false,
    };
    let mut buffer = [0u8; BINARY_SNIFF_BYTES];
    let bytes_read = match file.read(&mut buffer).await {
        Ok(bytes_read) => bytes_read,
        Err(_) => return false,
    };

    buffer[..bytes_read].contains(&0)
}

fn add_gitignore_matcher(
    scope_dir: &Path,
    gitignore_path: &Path,
    matchers: &mut Vec<(PathBuf, ignore::gitignore::Gitignore)>,
) {
    let mut builder = ignore::gitignore::GitignoreBuilder::new(scope_dir);
    if builder.add(gitignore_path).is_none() {
        if let Ok(matcher) = builder.build() {
            matchers.push((scope_dir.to_path_buf(), matcher));
        }
    }
}

pub(super) fn build_gitignore_matcher(
    search_root: &Path,
    workspace_root: &Path,
) -> Option<ScopedGitignoreMatcher> {
    use walkdir::WalkDir;

    if !search_root.is_dir() {
        return None;
    }

    let mut matchers = Vec::new();
    let root_gitignore = search_root.join(".gitignore");
    if root_gitignore.is_file() {
        add_gitignore_matcher(search_root, &root_gitignore, &mut matchers);
    }

    let walker = WalkDir::new(search_root)
        .into_iter()
        .filter_entry(|entry| {
            if !entry.file_type().is_dir() {
                return true;
            }

            entry.depth() == 0
                || (!is_internal_workspace_artifact_path(workspace_root, entry.path())
                    && entry
                        .file_name()
                        .to_str()
                        .map(|name| !SKIPPED_SEARCH_DIR_NAMES.contains(&name))
                        .unwrap_or(true))
        })
        .filter_map(Result::ok);

    for entry in walker {
        if entry.file_type().is_file()
            && entry.file_name() == OsStr::new(".gitignore")
            && entry.path() != root_gitignore
        {
            if let Some(scope_dir) = entry.path().parent() {
                add_gitignore_matcher(scope_dir, entry.path(), &mut matchers);
            }
        }
    }

    if matchers.is_empty() {
        return None;
    }

    Some(ScopedGitignoreMatcher {
        search_root: search_root.to_path_buf(),
        matchers,
    })
}

pub(super) fn reject_empty_optional_str<'a>(
    value: Option<&'a str>,
    field: &str,
    guidance: Vec<String>,
) -> Result<Option<&'a str>, MCPResult> {
    if matches!(value, Some("")) {
        return Err(guided_error(
            ErrorCategory::InvalidInput,
            format!("Invalid {field}: {field} must not be empty"),
            ToolGroup::Workspace,
        )
        .guidance(guidance)
        .to_mcp_result());
    }
    Ok(value)
}

pub(super) fn parse_pagination(args: &Value) -> Result<(usize, usize), MCPResult> {
    let limit_raw = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(50);
    if !(1..=1000).contains(&limit_raw) {
        return Err(guided_error(
            ErrorCategory::InvalidInput,
            format!(
                "Invalid pagination parameter: limit must be between 1 and 1000, got {limit_raw}"
            ),
            ToolGroup::Workspace,
        )
        .guidance(vec![
            "Set limit to a value between 1 and 1000".to_string(),
            "Use offset to paginate through additional results".to_string(),
        ])
        .to_mcp_result());
    }
    let limit = match usize::try_from(limit_raw) {
        Ok(value) => value,
        Err(_) => {
            return Err(guided_error(
                ErrorCategory::InvalidInput,
                format!(
                    "Invalid pagination parameter: limit is too large for this platform ({limit_raw})"
                ),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Set limit to a value between 1 and 1000".to_string(),
                "Use smaller page sizes when paginating search results".to_string(),
            ])
            .to_mcp_result());
        }
    };

    let offset_raw = args.get("offset").and_then(|v| v.as_u64()).unwrap_or(0);
    let offset = match usize::try_from(offset_raw) {
        Ok(value) => value,
        Err(_) => {
            return Err(guided_error(
                ErrorCategory::InvalidInput,
                format!(
                    "Invalid pagination parameter: offset is too large for this platform ({offset_raw})"
                ),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Set offset to a smaller non-negative value".to_string(),
                "Use limit and offset together to page through results".to_string(),
            ])
            .to_mcp_result());
        }
    };

    Ok((limit, offset))
}

pub(super) fn preview_grep_match_line(line: &str, regex: &regex::Regex) -> String {
    preview_grep_match_line_with_limit(line, regex, MAX_GREP_MATCH_LINE_CHARS)
}

fn preview_grep_match_line_with_limit(
    line: &str,
    regex: &regex::Regex,
    max_chars: usize,
) -> String {
    let total = line.chars().count();
    if max_chars == 0 {
        return format!("…[{total} chars total, line continues]");
    }
    if total <= max_chars {
        return line.to_string();
    }

    let (window_start, window_end) = match_window_char_range(line, regex, total, max_chars);
    let preview: String = line
        .chars()
        .skip(window_start)
        .take(window_end.saturating_sub(window_start))
        .collect();

    let mut rendered = String::new();
    if window_start > 0 {
        rendered.push('…');
    }
    rendered.push_str(&preview);
    if window_end < total {
        rendered.push_str(&format!(" …[{total} chars total, line continues]"));
    }
    rendered
}

fn match_window_char_range(
    line: &str,
    regex: &regex::Regex,
    total_chars: usize,
    max_chars: usize,
) -> (usize, usize) {
    let Some(matched) = regex.find(line) else {
        return (0, max_chars.min(total_chars));
    };

    let match_start = line[..matched.start()].chars().count();
    let match_len = line[matched.start()..matched.end()].chars().count();
    let match_end = match_start + match_len;

    if match_len >= max_chars {
        return (match_start, match_start + max_chars);
    }

    let extra = max_chars - match_len;
    let mut before = extra / 2;
    let mut after = extra - before;

    if match_start < before {
        after += before - match_start;
        before = match_start;
    }
    if total_chars - match_end < after {
        let leftover = after - (total_chars - match_end);
        after = total_chars - match_end;
        before = (before + leftover).min(match_start);
    }

    (match_start - before, match_end + after)
}

#[cfg(test)]
mod grep_match_preview_tests {
    use super::{preview_grep_match_line_with_limit, MAX_GREP_MATCH_LINE_CHARS};
    use regex::Regex;

    #[test]
    fn short_line_is_unchanged() {
        let regex = Regex::new("needle").unwrap();
        assert_eq!(
            preview_grep_match_line_with_limit("keep needle intact", &regex, 200),
            "keep needle intact"
        );
    }

    #[test]
    fn long_line_keeps_match_near_the_end() {
        let regex = Regex::new("hf_secret").unwrap();
        let line = format!("{}hf_secret{}", "x".repeat(8000), "y".repeat(8000));
        let preview = preview_grep_match_line_with_limit(&line, &regex, 40);
        assert!(preview.contains("hf_secret"), "{preview}");
        assert!(
            preview.contains("16009 chars total, line continues"),
            "{preview}"
        );
        assert!(!preview.contains(&"x".repeat(40)), "{preview}");
        assert!(preview.chars().count() < 120, "{}", preview.chars().count());
    }

    #[test]
    fn default_limit_is_two_hundred_chars() {
        assert_eq!(MAX_GREP_MATCH_LINE_CHARS, 200);
    }
}

#[cfg(test)]
mod brace_glob_tests {
    use super::{expand_brace_globs, matches_glob, GlobMatcher};
    use std::path::Path;

    #[test]
    fn expands_extension_list() {
        let mut got = expand_brace_globs("*.{py,yaml}").unwrap();
        got.sort();
        assert_eq!(got, vec!["*.py".to_string(), "*.yaml".to_string()]);
    }

    #[test]
    fn expands_nested_and_cartesian_groups() {
        let mut nested = expand_brace_globs("{a,{b,c}}").unwrap();
        nested.sort();
        assert_eq!(
            nested,
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );

        let mut cartesian = expand_brace_globs("{src,lib}/*.{ts,js}").unwrap();
        cartesian.sort();
        assert_eq!(
            cartesian,
            vec![
                "lib/*.js".to_string(),
                "lib/*.ts".to_string(),
                "src/*.js".to_string(),
                "src/*.ts".to_string(),
            ]
        );
    }

    #[test]
    fn literal_braces_without_comma_stay_unexpanded() {
        assert_eq!(
            expand_brace_globs("file{backup}").unwrap(),
            vec!["file{backup}".to_string()]
        );
    }

    #[test]
    fn unmatched_brace_is_an_error() {
        let err = expand_brace_globs("*.{py,yaml").unwrap_err();
        assert!(err.contains("unmatched"), "{err}");
    }

    #[test]
    fn matcher_accepts_any_expanded_alternative() {
        let matcher = GlobMatcher::parse("*.{rs,ts}").unwrap();
        assert!(matches_glob(
            &matcher,
            Path::new("src/main.rs"),
            Some("main.rs")
        ));
        assert!(matches_glob(
            &matcher,
            Path::new("src/main.ts"),
            Some("main.ts")
        ));
        assert!(!matches_glob(
            &matcher,
            Path::new("src/main.py"),
            Some("main.py")
        ));
    }
}
