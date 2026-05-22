use super::super::utils::{
    compute_anchor, detect_language, format_file_size, initial_prefix_hash_state,
    update_prefix_hash_state,
};
use super::helpers::*;
use crate::mcp::builtin::error_guidance::{guided_error, ErrorCategory, SuccessHint, ToolGroup};
use crate::mcp::types::MCPResult;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::Path;

pub(super) struct SearchContentRequest<'a> {
    pub display_path: &'a str,
    pub regex: &'a regex::Regex,
    pub query: &'a str,
    pub show_hashes: bool,
    pub ignore_case: bool,
    pub limit: usize,
    pub offset: usize,
}

pub(super) struct SearchDirectoryRequest<'a> {
    pub workspace_root: &'a Path,
    pub dir: &'a Path,
    pub file_pattern: Option<&'a glob::Pattern>,
    pub search: SearchContentRequest<'a>,
}

struct LineInfo {
    start: usize,
}

fn collect_line_infos(content: &str) -> Vec<LineInfo> {
    let mut line_infos = Vec::new();
    let mut cursor = 0usize;

    for line in content.lines() {
        line_infos.push(LineInfo { start: cursor });

        cursor += line.len();
        let remainder = &content[cursor..];
        if remainder.starts_with("\r\n") {
            cursor += 2;
        } else if remainder.starts_with('\n') || remainder.starts_with('\r') {
            cursor += 1;
        }
    }

    line_infos
}

fn line_index_for_offset(line_infos: &[LineInfo], offset: usize) -> Option<usize> {
    if line_infos.is_empty() {
        return None;
    }

    match line_infos.binary_search_by_key(&offset, |line| line.start) {
        Ok(index) => Some(index),
        Err(0) => Some(0),
        Err(index) => Some(index - 1),
    }
}

fn collect_matched_line_indices(content: &str, regex: &regex::Regex) -> BTreeSet<usize> {
    let line_infos = collect_line_infos(content);
    let mut matched_lines = BTreeSet::new();

    if line_infos.is_empty() {
        return matched_lines;
    }

    for matched in regex.find_iter(content) {
        let start_line = match line_index_for_offset(&line_infos, matched.start()) {
            Some(index) => index,
            None => continue,
        };

        let end_offset = if matched.start() == matched.end() {
            matched.start()
        } else {
            matched.end().saturating_sub(1)
        };
        let end_line = match line_index_for_offset(&line_infos, end_offset) {
            Some(index) => index,
            None => continue,
        };

        for line_index in start_line..=end_line {
            matched_lines.insert(line_index);
        }
    }

    matched_lines
}

pub(super) async fn search_content_in_file(
    file_path: &Path,
    request: SearchContentRequest<'_>,
) -> Result<MCPResult, String> {
    let SearchContentRequest {
        display_path,
        regex,
        query,
        show_hashes,
        ignore_case,
        limit,
        offset,
    } = request;

    let max_size = effective_search_content_file_size_limit();
    if let Ok(metadata) = tokio::fs::metadata(file_path).await {
        let file_size = metadata.len() as usize;
        if file_size > max_size {
            return Ok(guided_error(
                ErrorCategory::OperationFailed,
                format!(
                    "Failed to search file: file size {} exceeds the search limit of {}.",
                    format_file_size(file_size as u64),
                    format_file_size(max_size as u64)
                ),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Search a smaller file or narrow the directory before searching contents"
                    .to_string(),
                "Use readFile or listDirectory first to inspect large generated artifacts"
                    .to_string(),
            ])
            .to_mcp_result());
        }
    }
    if is_probably_binary_file(file_path).await {
        return Ok(guided_error(
            ErrorCategory::OperationFailed,
            format!(
                "Failed to search file: `{}` appears to be binary data.",
                display_path
            ),
            ToolGroup::Workspace,
        )
        .guidance(vec![
            "Search text files instead of binary artifacts".to_string(),
            "Use filePattern to narrow the directory before searching contents".to_string(),
        ])
        .to_mcp_result());
    }

    let content = match tokio::fs::read_to_string(file_path).await {
        Ok(s) => s,
        Err(e) => {
            let error_msg = if e.kind() == std::io::ErrorKind::InvalidData {
                "Failed to read file: Content appears to be binary or contains invalid UTF-8 characters.".to_string()
            } else {
                e.to_string()
            };
            return Ok(guided_error(
                ErrorCategory::OperationFailed,
                &error_msg,
                ToolGroup::Workspace,
            )
            .to_mcp_result());
        }
    };

    let matched_lines = collect_matched_line_indices(&content, regex);
    let mut matches = Vec::new();
    let mut prefix_state = initial_prefix_hash_state();
    for (idx, line) in content.lines().enumerate() {
        if matched_lines.contains(&idx) {
            if show_hashes {
                let anchor = compute_anchor(line, &mut prefix_state);
                matches.push(json!({
                    "line": idx + 1,
                    "anchor": anchor,
                    "text": line
                }));
            } else {
                matches.push(json!({ "line": idx + 1, "text": line }));
                prefix_state = update_prefix_hash_state(prefix_state, line);
            }
        } else {
            prefix_state = update_prefix_hash_state(prefix_state, line);
        }
    }

    let language = detect_language(file_path);

    let total_matches = matches.len();
    let paginated_matches: Vec<_> = matches.into_iter().skip(offset).take(limit).collect();
    let has_more = offset + paginated_matches.len() < total_matches;

    let text_output = if total_matches == 0 {
        format!(
            "**🔍 Search Results: No matches found**\n\n\
            Pattern: `{}`\n\
            File: `{}`\n\
            Options: {}\n",
            query,
            display_path,
            if ignore_case {
                "case-insensitive"
            } else {
                "case-sensitive"
            }
        )
    } else {
        let mut s = format!(
            "**🔍 Search Results: {} match(es) found**\n\n",
            total_matches
        );
        s.push_str(&format!(
            "File: `{}`\nPattern: `{}`\n\n",
            display_path, query
        ));

        s.push_str("```");
        s.push_str(language);
        s.push('\n');

        for m in &paginated_matches {
            let line_num = m.get("line").and_then(|v| v.as_u64()).unwrap_or(0);
            let text = m.get("text").and_then(|v| v.as_str()).unwrap_or("");
            if let Some(anchor) = m.get("anchor").and_then(|v| v.as_str()) {
                s.push_str(&format!("{}:{}|{}\n", line_num, anchor, text));
            } else {
                s.push_str(&format!("Line {}: {}\n", line_num, text));
            }
        }
        s.push_str("```\n\n");

        if has_more {
            s.push_str(&format!(
                "*(Showing {} to {} of {} total matches. Call search with offset: {} to see more)*\n\n",
                offset + 1,
                offset + paginated_matches.len(),
                total_matches,
                offset + limit
            ));
        } else if offset > 0 {
            s.push_str(&format!(
                "*(Showing {} to {} of {} total matches)*\n\n",
                offset + 1,
                offset + paginated_matches.len(),
                total_matches
            ));
        }

        if show_hashes {
            s.push_str("Use the returned anchors with editFiles. For range replacement/deletion, also copy endAnchor from the exact end line.\n");
        } else {
            s.push_str(
                "If you plan to use editFiles next, run again with `showLineAnchors: true` to get anchors.\n",
            );
        }
        s
    };

    Ok(MCPResult::success_with_data(
        &text_output,
        json!({
            "matches": paginated_matches,
            "total_matches": total_matches,
            "offset": offset,
            "limit": limit
        }),
    ))
}

pub(super) async fn search_content_in_dir(
    request: SearchDirectoryRequest<'_>,
) -> Result<MCPResult, String> {
    use walkdir::WalkDir;

    let SearchDirectoryRequest {
        workspace_root,
        dir,
        file_pattern,
        search:
            SearchContentRequest {
                display_path,
                regex,
                query,
                show_hashes,
                ignore_case,
                limit,
                offset,
            },
    } = request;

    struct FileMatch {
        rel_path: String,
        hits: Vec<Value>,
    }

    let mut file_matches: Vec<FileMatch> = Vec::new();
    let mut files_searched: usize = 0;
    let mut skipped_heavy_dirs = 0usize;
    let mut skipped_gitignored_dirs = 0usize;
    let mut skipped_binary_files = 0usize;
    let mut skipped_large_files = 0usize;
    let max_size = effective_search_content_file_size_limit();
    let gitignore = build_gitignore_matcher(dir, workspace_root);

    let walker = WalkDir::new(dir)
        .into_iter()
        .filter_entry(|entry| {
            if let Some(reason) =
                classify_search_entry_skip(workspace_root, entry, gitignore.as_ref())
            {
                if entry.file_type().is_dir() {
                    match reason {
                        SearchEntrySkipReason::Gitignored => skipped_gitignored_dirs += 1,
                        SearchEntrySkipReason::HeavyweightDirectory => skipped_heavy_dirs += 1,
                        SearchEntrySkipReason::InternalArtifactDirectory => {}
                    }
                }
                return false;
            }

            true
        })
        .filter_map(|e| e.ok());

    for entry in walker {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let file_name = path.file_name().and_then(|n| n.to_str());
        let rel_path_obj = path.strip_prefix(dir).unwrap_or(path);

        if let Some(glob_pat) = file_pattern {
            if !matches_glob(glob_pat, rel_path_obj, file_name) {
                continue;
            }
        }

        // Skip obviously binary extensions
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        if matches!(
            ext.as_str(),
            "png"
                | "jpg"
                | "jpeg"
                | "gif"
                | "webp"
                | "svg"
                | "ico"
                | "pdf"
                | "zip"
                | "tar"
                | "gz"
                | "bz2"
                | "xz"
                | "exe"
                | "dll"
                | "so"
                | "dylib"
                | "bin"
                | "wasm"
                | "mp3"
                | "mp4"
                | "wav"
                | "ogg"
                | "flac"
                | "ttf"
                | "woff"
                | "woff2"
        ) {
            continue;
        }

        let file_size = match entry.metadata() {
            Ok(metadata) => metadata.len() as usize,
            Err(_) => continue,
        };
        if file_size > max_size {
            skipped_large_files += 1;
            continue;
        }
        if is_probably_binary_file(path).await {
            skipped_binary_files += 1;
            continue;
        }

        let content = match tokio::fs::read_to_string(path).await {
            Ok(s) => s,
            Err(_) => continue, // skip unreadable
        };
        files_searched += 1;

        let rel_path = {
            let p = rel_path_obj.to_string_lossy().to_string();
            #[cfg(target_os = "windows")]
            let p = p.replace('\\', "/");
            p
        };

        let matched_lines = collect_matched_line_indices(&content, regex);
        let mut hits = Vec::new();
        let mut prefix_state = initial_prefix_hash_state();
        for (idx, line) in content.lines().enumerate() {
            if matched_lines.contains(&idx) {
                if show_hashes {
                    let anchor = compute_anchor(line, &mut prefix_state);
                    hits.push(json!({
                        "line": idx + 1,
                        "anchor": anchor,
                        "text": line
                    }));
                } else {
                    hits.push(json!({ "line": idx + 1, "text": line }));
                    prefix_state = update_prefix_hash_state(prefix_state, line);
                }
            } else {
                prefix_state = update_prefix_hash_state(prefix_state, line);
            }
        }

        if !hits.is_empty() {
            file_matches.push(FileMatch { rel_path, hits });
        }
    }

    let options_str = if ignore_case {
        "case-insensitive"
    } else {
        "case-sensitive"
    };

    if file_matches.is_empty() {
        let mut next_steps = vec![
            "Try a broader pattern or check the directory path".to_string(),
            "Try toggling ignoreCase".to_string(),
        ];
        if skipped_large_files > 0 {
            next_steps.push(format!(
                "Skipped {} large file(s) over {}; narrow the search path if you need those files",
                skipped_large_files,
                format_file_size(max_size as u64)
            ));
        }
        if skipped_binary_files > 0 {
            next_steps.push(format!(
                "Skipped {} binary-looking file(s); refine filePattern if you need a specific artifact",
                skipped_binary_files
            ));
        }
        return Ok(SuccessHint::new(
            format!(
                "No matches for `{}` in {} file(s) under `{}` (Options: {})",
                query, files_searched, display_path, options_str
            ),
            next_steps,
        )
        .to_mcp_result());
    }

    let total_hits: usize = file_matches.iter().map(|f| f.hits.len()).sum();

    let mut text = format!(
        "**🔍 Directory Search: {} match(es) in {} file(s)** (searched {} files)\n\
         Pattern: `{}`  Path: `{}`\n\
         Options: {}\n\n",
        total_hits,
        file_matches.len(),
        files_searched,
        query,
        display_path,
        options_str
    );

    let total_files = file_matches.len();
    let paginated_files: Vec<_> = file_matches.into_iter().skip(offset).take(limit).collect();
    let has_more = offset + paginated_files.len() < total_files;

    let per_file_preview_limit = 5usize;
    for fm in &paginated_files {
        text.push_str(&format!("### `{}`\n", fm.rel_path));
        for hit in fm.hits.iter().take(per_file_preview_limit) {
            let line_num = hit.get("line").and_then(|v| v.as_u64()).unwrap_or(0);
            let t = hit.get("text").and_then(|v| v.as_str()).unwrap_or("");
            if let Some(anchor) = hit.get("anchor").and_then(|v| v.as_str()) {
                text.push_str(&format!("{}:{}|{}\n", line_num, anchor, t));
            } else {
                text.push_str(&format!("- L{}: `{}`\n", line_num, t.trim()));
            }
        }
        if fm.hits.len() > per_file_preview_limit {
            text.push_str(&format!(
                "  ... and {} more matches in this file (showing first {} per file)\n",
                fm.hits.len() - per_file_preview_limit,
                per_file_preview_limit
            ));
        }
        text.push('\n');
    }

    if has_more {
        text.push_str(&format!(
            "*(Showing {} to {} of {} total files with matches. Call search with offset: {} to see more)*\n",
            offset + 1,
            offset + paginated_files.len(),
            total_files,
            offset + limit
        ));
    } else if offset > 0 {
        text.push_str(&format!(
            "*(Showing {} to {} of {} total files with matches)*\n",
            offset + 1,
            offset + paginated_files.len(),
            total_files
        ));
    }
    if skipped_heavy_dirs > 0
        || skipped_gitignored_dirs > 0
        || skipped_large_files > 0
        || skipped_binary_files > 0
    {
        text.push('\n');
    }
    if skipped_heavy_dirs > 0 {
        text.push_str(&format!(
            "*Skipped {} heavyweight director{} (`{}`)*\n",
            skipped_heavy_dirs,
            if skipped_heavy_dirs == 1 { "y" } else { "ies" },
            SKIPPED_SEARCH_DIR_NAMES.join("`, `")
        ));
    }
    if skipped_gitignored_dirs > 0 {
        text.push_str(&format!(
            "*Skipped {} .gitignore-matched director{}*\n",
            skipped_gitignored_dirs,
            if skipped_gitignored_dirs == 1 {
                "y"
            } else {
                "ies"
            },
        ));
    }
    if skipped_large_files > 0 {
        text.push_str(&format!(
            "*Skipped {} large file(s) over {}*\n",
            skipped_large_files,
            format_file_size(max_size as u64)
        ));
    }
    if skipped_binary_files > 0 {
        text.push_str(&format!(
            "*Skipped {} binary-looking file(s)*\n",
            skipped_binary_files
        ));
    }

    let structured = json!({
        "pattern": query,
        "directory": display_path,
        "files_searched": files_searched,
        "files_with_matches": total_files,
        "total_matches": total_hits,
        "offset": offset,
        "limit": limit,
        "skipped_directories": skipped_heavy_dirs + skipped_gitignored_dirs,
        "skipped_heavyweight_directories": skipped_heavy_dirs,
        "skipped_gitignored_directories": skipped_gitignored_dirs,
        "skipped_binary_files": skipped_binary_files,
        "skipped_large_files": skipped_large_files,
        "max_file_size": max_size,
        "results": paginated_files.iter().map(|fm| json!({
            "file": fm.rel_path,
            "matches": fm.hits,
        })).collect::<Vec<_>>(),
    });

    let mut next_steps =
        vec!["Use search with a specific file path to see all matches in that file".to_string()];
    if show_hashes {
        next_steps.push(
            "Use the returned anchors with editFiles; add endAnchor for range replacement/deletion"
                .to_string(),
        );
    } else {
        next_steps.push(
            "Run with `showLineAnchors: true` to get anchors for targeted editing tools."
                .to_string(),
        );
    }

    Ok(SuccessHint::new(text, next_steps).to_mcp_result_with_data(Some(structured)))
}
