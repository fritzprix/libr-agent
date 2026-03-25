use super::super::WorkspaceServer;
use super::utils::{compute_line_hash, detect_language, format_file_size};
use crate::mcp::builtin::error_guidance::{
    guided_error, missing_param_error, ErrorCategory, SuccessHint, ToolGroup,
};
use crate::mcp::types::MCPResult;
use serde_json::{json, Value};
use std::path::Path;

impl WorkspaceServer {
    pub async fn handle_search(
        &self,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        let search_path = match args.get("path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return Ok(missing_param_error("path", ToolGroup::Workspace)),
        };

        let query = args.get("query").and_then(|v| v.as_str());
        let file_pattern = args.get("filePattern").and_then(|v| v.as_str());
        let ignore_case = args
            .get("ignoreCase")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let show_line_hashes = args
            .get("showLineHashes")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Security validation
        let file_manager = self.get_file_manager(session_id.clone());
        let safe_path = match file_manager
            .get_security_validator()
            .validate_path_for_read(search_path)
        {
            Ok(path) => path,
            Err(e) => {
                return Ok(guided_error(
                    ErrorCategory::PermissionDenied,
                    format!("Path validation failed: {}", e),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Verify the file path is within allowed directories".to_string(),
                    "Use listDirectory to see available files".to_string(),
                ])
                .to_mcp_result());
            }
        };

        // Determine if we are doing file name search only or content search
        if query.is_none() {
            // File Name Search Only
            let pattern = match file_pattern {
                Some(p) => p,
                None => {
                    return Ok(guided_error(
                        ErrorCategory::MissingRequiredParam,
                        "Provide either 'query' (to search text) or 'filePattern' (to find files)"
                            .to_string(),
                        ToolGroup::Workspace,
                    )
                    .to_mcp_result());
                }
            };
            return self
                .search_files_only(&safe_path, search_path, pattern)
                .await;
        }

        // Text Content Search
        let query_str = query.unwrap();
        let regex = match regex::RegexBuilder::new(query_str)
            .case_insensitive(ignore_case)
            .build()
        {
            Ok(r) => r,
            Err(e) => {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    format!("Invalid regex pattern: {}", e),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Check regex syntax - use basic patterns like 'error|warning'".to_string(),
                    "Escape special characters with backslash: \\. \\* \\+ \\?".to_string(),
                ])
                .to_mcp_result());
            }
        };

        let glob_pat = match file_pattern {
            Some(p) => match glob::Pattern::new(p) {
                Ok(pat) => Some(pat),
                Err(e) => {
                    return Ok(guided_error(
                        ErrorCategory::InvalidInput,
                        format!("Invalid filePattern: {}", e),
                        ToolGroup::Workspace,
                    )
                    .to_mcp_result());
                }
            },
            None => None,
        };

        if safe_path.is_dir() {
            self.search_content_in_dir(
                &safe_path,
                search_path,
                &regex,
                query_str,
                glob_pat.as_ref(),
                show_line_hashes,
                ignore_case,
            )
            .await
        } else {
            self.search_content_in_file(
                &safe_path,
                search_path,
                &regex,
                query_str,
                show_line_hashes,
                ignore_case,
            )
            .await
        }
    }

    async fn search_files_only(
        &self,
        root_path: &Path,
        display_path: &str,
        pattern: &str,
    ) -> Result<MCPResult, String> {
        use walkdir::WalkDir;

        let glob_pattern = match glob::Pattern::new(pattern) {
            Ok(pat) => pat,
            Err(e) => {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    format!("Invalid pattern: {}", e),
                    ToolGroup::Workspace,
                )
                .to_mcp_result());
            }
        };

        let mut results = Vec::new();

        // Check if root_path itself is a file
        if root_path.is_file() {
            let file_name = root_path.file_name().and_then(|n| n.to_str());
            if matches_glob(&glob_pattern, root_path, file_name) {
                let size = tokio::fs::metadata(root_path)
                    .await
                    .map(|m| m.len())
                    .unwrap_or(0);
                results.push(json!({
                    "path": display_path,
                    "type": "file",
                    "size": size
                }));
            }
        } else {
            for entry in WalkDir::new(root_path).into_iter().filter_map(|e| e.ok()) {
                let path = entry.path();
                let is_dir = path.is_dir();
                let is_file = path.is_file();

                if !is_file && !is_dir {
                    continue;
                }

                let file_name = path.file_name().and_then(|n| n.to_str());
                let relative_path = path.strip_prefix(root_path).unwrap_or(path);

                if matches_glob(&glob_pattern, relative_path, file_name) {
                    let path_str = {
                        let p = relative_path.to_string_lossy().to_string();
                        #[cfg(target_os = "windows")]
                        let p = p.replace('\\', "/");
                        p
                    };

                    let size = if is_file {
                        entry.metadata().ok().map(|m| m.len())
                    } else {
                        None
                    };

                    results.push(json!({
                        "path": path_str,
                        "type": if is_dir { "directory" } else { "file" },
                        "size": size
                    }));
                }
            }
        }

        let result_text = if results.is_empty() {
            format!(
                "**🔍 File Search: No matches found**\n\n\
                Pattern: `{}`\n\
                Search Path: `{}`\n\n\
                **Next Steps:**\n\
                - Verify the pattern syntax (use glob format like `*.txt` or `**/*.rs`)\n\
                - Use listDirectory to explore available files",
                pattern, display_path
            )
        } else {
            let mut text = format!(
                "**🔍 File Search: {} file(s) found**\n\n\
                Pattern: `{}`\n\
                Search Path: `{}`\n\n",
                results.len(),
                pattern,
                display_path
            );

            text.push_str("**Matches:**\n");
            for item in results.iter().take(50) {
                let p = item.get("path").and_then(|v| v.as_str()).unwrap_or("?");
                let t = item.get("type").and_then(|v| v.as_str()).unwrap_or("?");
                let icon = if t == "file" { "📄" } else { "📁" };
                if let Some(size) = item.get("size").and_then(|v| v.as_u64()) {
                    text.push_str(&format!(
                        "- {} `{}` ({})\n",
                        icon,
                        p,
                        format_file_size(size)
                    ));
                } else {
                    text.push_str(&format!("- {} `{}`\n", icon, p));
                }
            }

            if results.len() > 50 {
                text.push_str(&format!(
                    "\n*Showing first 50 of {} total matches*\n",
                    results.len()
                ));
            }
            text
        };

        Ok(MCPResult::success_with_data(
            &result_text,
            json!({ "matches": results }),
        ))
    }

    async fn search_content_in_file(
        &self,
        file_path: &Path,
        display_path: &str,
        regex: &regex::Regex,
        query: &str,
        show_hashes: bool,
        ignore_case: bool,
    ) -> Result<MCPResult, String> {
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

        let mut matches = Vec::new();
        for (idx, line) in content.lines().enumerate() {
            if regex.is_match(line) {
                if show_hashes {
                    matches.push(json!({
                        "line": idx + 1,
                        "hash": compute_line_hash(line),
                        "text": line
                    }));
                } else {
                    matches.push(json!({ "line": idx + 1, "text": line }));
                }
            }
        }

        let language = detect_language(file_path);

        let text_output = if matches.is_empty() {
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
                matches.len()
            );
            s.push_str(&format!(
                "File: `{}`\nPattern: `{}`\n\n",
                display_path, query
            ));

            let matches_to_show = matches.len().min(50);
            s.push_str("```");
            s.push_str(language);
            s.push('\n');

            for m in matches.iter().take(matches_to_show) {
                let line_num = m.get("line").and_then(|v| v.as_u64()).unwrap_or(0);
                let text = m.get("text").and_then(|v| v.as_str()).unwrap_or("");
                if let Some(hash) = m.get("hash").and_then(|v| v.as_str()) {
                    s.push_str(&format!("{}:{}|{}\n", line_num, hash, text));
                } else {
                    s.push_str(&format!("Line {}: {}\n", line_num, text));
                }
            }
            s.push_str("```\n\n");

            if matches.len() > 50 {
                s.push_str(&format!(
                    "*Showing first 50 of {} total matches*\n\n",
                    matches.len()
                ));
            }

            if show_hashes {
                s.push_str("Use the format `N:hash|content` for `editFile`.\n");
            } else {
                s.push_str("Run with `showLineHashes: true` to get hashes for `editFile`.\n");
            }
            s
        };

        Ok(MCPResult::success_with_data(
            &text_output,
            json!({ "matches": matches }),
        ))
    }

    #[allow(clippy::too_many_arguments)]
    async fn search_content_in_dir(
        &self,
        dir: &Path,
        display_path: &str,
        regex: &regex::Regex,
        query: &str,
        file_pattern: Option<&glob::Pattern>,
        show_hashes: bool,
        ignore_case: bool,
    ) -> Result<MCPResult, String> {
        use walkdir::WalkDir;

        struct FileMatch {
            rel_path: String,
            hits: Vec<Value>,
        }

        let mut file_matches: Vec<FileMatch> = Vec::new();
        let mut files_searched: usize = 0;

        for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
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

            let mut hits = Vec::new();
            for (idx, line) in content.lines().enumerate() {
                if regex.is_match(line) {
                    if show_hashes {
                        hits.push(json!({
                            "line": idx + 1,
                            "hash": compute_line_hash(line),
                            "text": line
                        }));
                    } else {
                        hits.push(json!({ "line": idx + 1, "text": line }));
                    }
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
            return Ok(SuccessHint::new(
                format!(
                    "No matches for `{}` in {} file(s) under `{}` (Options: {})",
                    query, files_searched, display_path, options_str
                ),
                vec![
                    "Try a broader pattern or check the directory path".to_string(),
                    "Try toggling ignoreCase".to_string(),
                ],
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

        for fm in file_matches.iter().take(10) {
            text.push_str(&format!("### `{}`\n", fm.rel_path));
            for hit in fm.hits.iter().take(5) {
                let line_num = hit.get("line").and_then(|v| v.as_u64()).unwrap_or(0);
                let t = hit.get("text").and_then(|v| v.as_str()).unwrap_or("");
                if let Some(hash) = hit.get("hash").and_then(|v| v.as_str()) {
                    text.push_str(&format!("- {}:{}|`{}`\n", line_num, hash, t.trim()));
                } else {
                    text.push_str(&format!("- L{}: `{}`\n", line_num, t.trim()));
                }
            }
            if fm.hits.len() > 5 {
                text.push_str(&format!(
                    "  ... and {} more matches in this file\n",
                    fm.hits.len() - 5
                ));
            }
            text.push('\n');
        }

        if file_matches.len() > 10 {
            text.push_str(&format!(
                "*... and {} more files with matches*\n",
                file_matches.len() - 10
            ));
        }

        let structured = json!({
            "pattern": query,
            "directory": display_path,
            "files_searched": files_searched,
            "files_with_matches": file_matches.len(),
            "total_matches": total_hits,
            "results": file_matches.iter().map(|fm| json!({
                "file": fm.rel_path,
                "matches": fm.hits,
            })).collect::<Vec<_>>(),
        });

        let mut next_steps = vec![
            "Use search with a specific file path to see all matches in that file".to_string(),
        ];
        if !show_hashes {
            next_steps
                .push("Run with `showLineHashes: true` to get hashes for `editFile`.".to_string());
        }

        Ok(SuccessHint::new(text, next_steps).to_mcp_result_with_data(Some(structured)))
    }
}

/// Helper function to match paths against glob patterns in a cross-platform way.
fn matches_glob(pattern: &glob::Pattern, path: &Path, file_name: Option<&str>) -> bool {
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
