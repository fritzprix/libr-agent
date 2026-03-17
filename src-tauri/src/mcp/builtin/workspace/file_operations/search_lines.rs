use super::super::WorkspaceServer;
use super::utils::detect_language;
use crate::mcp::builtin::error_guidance::{
    guided_error, missing_param_error, ErrorCategory, SuccessHint, ToolGroup,
};
use crate::mcp::types::MCPResult;
use serde_json::{json, Value};

impl WorkspaceServer {
    pub async fn handle_search_lines(
        &self,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        let pattern = match args.get("pattern").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return Ok(missing_param_error("pattern", ToolGroup::Workspace)),
        };

        let ignore_case = args
            .get("ignoreCase")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let line_numbers = args
            .get("lineNumbers")
            .and_then(|v| v.as_bool())
            .unwrap_or(true); // Default to true as per tool definition

        let input_text = if let Some(path_str) = args.get("path").and_then(|v| v.as_str()) {
            let file_manager = self.get_file_manager(session_id);
            match file_manager
                .get_security_validator()
                .validate_path_for_read(path_str)
            {
                Ok(safe_path) => {
                    // If the path is a directory, delegate to multi-file search
                    if safe_path.is_dir() {
                        return self
                            .search_lines_in_dir(
                                safe_path,
                                path_str,
                                pattern,
                                ignore_case,
                                line_numbers,
                            )
                            .await;
                    }
                    match tokio::fs::read_to_string(&safe_path).await {
                        Ok(s) => s,
                        Err(e) => {
                            let error_msg = if e.kind() == std::io::ErrorKind::InvalidData {
                                "Failed to read file: Content appears to be binary or contains invalid UTF-8 characters. Please use a specialized tool for binary files.".to_string()
                            } else {
                                e.to_string()
                            };

                            return Ok(guided_error(
                                ErrorCategory::OperationFailed,
                                &error_msg,
                                ToolGroup::Workspace,
                            )
                            .guidance(vec![
                                "Verify the file exists with listDirectory".to_string(),
                                "Check file permissions".to_string(),
                                "Ensure the path is correct".to_string(),
                            ])
                            .to_mcp_result());
                        }
                    }
                }
                Err(e) => {
                    return Ok(guided_error(
                        ErrorCategory::PermissionDenied,
                        format!("Path validation failed: {}", e),
                        ToolGroup::Workspace,
                    )
                    .guidance(vec![
                        "Verify the file path is within allowed directories".to_string(),
                        "Use listDirectory to see available files".to_string(),
                        "Check that the path doesn't contain '..' or absolute paths outside workspace".to_string(),
                    ])
                    .to_mcp_result());
                }
            }
        } else if let Some(s) = args.get("input").and_then(|v| v.as_str()) {
            s.to_string()
        } else {
            return Ok(guided_error(
                ErrorCategory::MissingRequiredParam,
                "Either 'path' or 'input' parameter must be provided".to_string(),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Use 'path' to search within a file".to_string(),
                "Use 'input' to search within provided text".to_string(),
                "Example: {\"pattern\": \"error\", \"path\": \"logs.txt\"}".to_string(),
            ])
            .to_mcp_result());
        };

        let regex = match regex::RegexBuilder::new(pattern)
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
                    "Test pattern with a simpler string first".to_string(),
                ])
                .to_mcp_result());
            }
        };

        let mut matches = Vec::new();
        let lines: Vec<&str> = input_text.lines().collect();

        for (idx, line) in lines.iter().enumerate() {
            if regex.is_match(line) {
                if line_numbers {
                    matches.push(json!({ "line": idx + 1, "text": line }));
                } else {
                    matches.push(json!(line));
                }
            }
        }

        let file_path = args.get("path").and_then(|v| v.as_str());
        let language = file_path
            .map(|p| detect_language(std::path::Path::new(p)))
            .unwrap_or("");

        let text_output = if matches.is_empty() {
            format!(
                "**🔍 Search Results: No matches found**\n\n\
                Pattern: `{}`\n\
                Options: {}\n\n\
                **Next Steps:**\n\
                - Try a different search pattern\n\
                - Use ignoreCase: true for case-insensitive search\n\
                - Check if the file contains the expected content with readFile",
                pattern,
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

            if let Some(path) = file_path {
                s.push_str(&format!("File: `{}`\n", path));
            }
            s.push_str(&format!("Pattern: `{}`\n", pattern));
            s.push_str(&format!(
                "Options: {}\n\n",
                if ignore_case {
                    "case-insensitive"
                } else {
                    "case-sensitive"
                }
            ));

            // Show up to 20 matches with context
            let matches_to_show = matches.len().min(20);
            s.push_str("```");
            if !language.is_empty() {
                s.push_str(language);
            }
            s.push('\n');

            for match_item in matches.iter().take(matches_to_show) {
                if let Some(obj) = match_item.as_object() {
                    if let Some(line_num) = obj.get("line").and_then(|v| v.as_u64()) {
                        let line_content = obj.get("text").and_then(|t| t.as_str()).unwrap_or("");

                        // ✅ ENHANCED: Explicitly format line number in text output
                        // Format: "Line 123: content"
                        s.push_str(&format!("Line {}: {}\n", line_num, line_content));
                    } else if let Some(text) = obj.get("text").and_then(|t| t.as_str()) {
                        s.push_str(&format!("{}\n", text));
                    }
                } else if let Some(str_val) = match_item.as_str() {
                    s.push_str(&format!("{}\n", str_val));
                }
            }

            s.push_str("```\n\n");

            if matches.len() > 20 {
                s.push_str(&format!(
                    "*Showing first 20 of {} total matches*\n\n",
                    matches.len()
                ));
            }

            s.push_str(
                "**Next Steps:**\n\
                - Use readFile to see full file context\n\
                - Use replaceLines to modify matched content\n\
                - Refine search pattern for more specific results",
            );

            s
        };

        Ok(MCPResult::success_with_data(
            &text_output,
            json!({ "matches": matches }),
        ))
    }

    /// Search for pattern matches across all text files in a directory (recursive).
    /// Called by `handle_search_lines` when the path resolves to a directory.
    async fn search_lines_in_dir(
        &self,
        dir: std::path::PathBuf,
        display_path: &str,
        pattern: &str,
        ignore_case: bool,
        line_numbers: bool,
    ) -> Result<MCPResult, String> {
        use walkdir::WalkDir;

        let regex = match regex::RegexBuilder::new(pattern)
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
                    "Check regex syntax — use basic patterns like 'error|warning'".to_string(),
                    "Escape special characters with backslash: \\. \\* \\+ \\?".to_string(),
                ])
                .to_mcp_result());
            }
        };

        // Collect per-file matches; skip binary / unreadable files silently.
        struct FileMatch {
            rel_path: String,
            hits: Vec<Value>,
        }

        let mut file_matches: Vec<FileMatch> = Vec::new();
        let mut files_searched: usize = 0;

        for entry in WalkDir::new(&dir).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if !path.is_file() {
                continue;
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
                Err(_) => continue, // binary or unreadable — skip silently
            };
            files_searched += 1;

            let rel_path = {
                let p = path
                    .strip_prefix(&dir)
                    .unwrap_or(path)
                    .to_string_lossy()
                    .to_string();
                #[cfg(target_os = "windows")]
                let p = p.replace('\\', "/");
                p
            };

            let mut hits: Vec<Value> = Vec::new();
            for (idx, line) in content.lines().enumerate() {
                if regex.is_match(line) {
                    if line_numbers {
                        hits.push(json!({ "line": idx + 1, "text": line }));
                    } else {
                        hits.push(json!(line));
                    }
                }
            }

            if !hits.is_empty() {
                file_matches.push(FileMatch { rel_path, hits });
            }
        }

        if file_matches.is_empty() {
            return Ok(SuccessHint::new(
                format!(
                    "No matches for `{}` in {} file(s) under `{}`",
                    pattern, files_searched, display_path
                ),
                vec![
                    "Try a broader pattern or check the directory path".to_string(),
                    "Use ignoreCase: true for case-insensitive search".to_string(),
                ],
            )
            .to_mcp_result());
        }

        let total_hits: usize = file_matches.iter().map(|f| f.hits.len()).sum();

        // Build human-readable text block
        let mut text = format!(
            "**🔍 Directory Search: {} match(es) in {} file(s)** (searched {} files)\n\
             Pattern: `{}`  Path: `{}`\n\n",
            total_hits,
            file_matches.len(),
            files_searched,
            pattern,
            display_path,
        );

        for fm in &file_matches {
            text.push_str(&format!("### `{}`\n", fm.rel_path));
            for hit in &fm.hits {
                if line_numbers {
                    let ln = hit.get("line").and_then(|v| v.as_u64()).unwrap_or(0);
                    let t = hit.get("text").and_then(|v| v.as_str()).unwrap_or("");
                    let lang = detect_language(std::path::Path::new(&fm.rel_path));
                    text.push_str(&format!("- L{}: `{}`\n", ln, t.trim()));
                    let _ = lang; // used for future syntax hint
                } else {
                    let t = hit.as_str().unwrap_or("");
                    text.push_str(&format!("- `{}`\n", t.trim()));
                }
            }
            text.push('\n');
        }

        let structured = json!({
            "pattern": pattern,
            "directory": display_path,
            "files_searched": files_searched,
            "files_with_matches": file_matches.len(),
            "total_matches": total_hits,
            "results": file_matches.iter().map(|fm| json!({
                "file": fm.rel_path,
                "matches": fm.hits,
            })).collect::<Vec<_>>(),
        });

        Ok(SuccessHint::new(
            text,
            vec!["Use path: \"file\" to narrow search to a specific file".to_string()],
        )
        .to_mcp_result_with_data(Some(structured)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::SessionManager;
    use serde_json::json;
    use std::sync::Arc;
    use tempfile::tempdir;

    // ── helpers ──────────────────────────────────────────────────────────────

    async fn create_server() -> (WorkspaceServer, tempfile::TempDir) {
        let tmp = tempdir().unwrap();
        let session_manager =
            Arc::new(SessionManager::new_with_base_dir(tmp.path().to_path_buf()).unwrap());
        let server = WorkspaceServer::new("test-session".to_string(), session_manager);
        (server, tmp)
    }

    // ── searchLines — directory path tests ───────────────────────────────────

    /// Basic happy path: two text files in a directory, both with matches.
    #[tokio::test]
    async fn test_search_lines_dir_basic() {
        let (server, tmp) = create_server().await;
        let dir = tmp.path().join("src");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.ts"), "const foo = 1;\nconst bar = 2;\n").unwrap();
        std::fs::write(dir.join("b.ts"), "let foo = true;\n").unwrap();

        let result = server
            .handle_search_lines(
                json!({ "path": dir.to_string_lossy(), "pattern": "foo" }),
                None,
            )
            .await
            .unwrap();

        let text = result
            .content
            .as_deref()
            .unwrap_or_default()
            .iter()
            .find_map(|c| {
                if let crate::mcp::types::MCPContent::Text { text, .. } = c {
                    Some(text.clone())
                } else {
                    None
                }
            })
            .unwrap_or_default();

        assert!(text.contains("2 match"), "expected 2 matches, got: {text}");
        assert!(
            text.contains("a.ts") || text.contains("b.ts"),
            "expected file names in output"
        );
        assert!(!result.is_error.unwrap_or(false));
    }

    /// No matches in directory returns a friendly no-match message, not an error.
    #[tokio::test]
    async fn test_search_lines_dir_no_match() {
        let (server, tmp) = create_server().await;
        let dir = tmp.path().join("empty_src");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("readme.txt"), "hello world\n").unwrap();

        let result = server
            .handle_search_lines(
                json!({ "path": dir.to_string_lossy(), "pattern": "zzznomatch" }),
                None,
            )
            .await
            .unwrap();

        assert!(!result.is_error.unwrap_or(false));
        let text = result
            .content
            .as_deref()
            .unwrap_or_default()
            .iter()
            .find_map(|c| {
                if let crate::mcp::types::MCPContent::Text { text, .. } = c {
                    Some(text.clone())
                } else {
                    None
                }
            })
            .unwrap_or_default();
        assert!(
            text.contains("No matches") || text.contains("no match"),
            "got: {text}"
        );
    }

    /// Binary files (e.g. .png) are silently skipped and do not cause errors.
    #[tokio::test]
    async fn test_search_lines_dir_skips_binary_extension() {
        let (server, tmp) = create_server().await;
        let dir = tmp.path().join("mixed");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("image.png"), b"\x89PNG\r\n\x1a\n fake binary").unwrap();
        std::fs::write(dir.join("code.ts"), "const needle = 42;\n").unwrap();

        let result = server
            .handle_search_lines(
                json!({ "path": dir.to_string_lossy(), "pattern": "needle" }),
                None,
            )
            .await
            .unwrap();

        assert!(!result.is_error.unwrap_or(false));
        let text = result
            .content
            .as_deref()
            .unwrap_or_default()
            .iter()
            .find_map(|c| {
                if let crate::mcp::types::MCPContent::Text { text, .. } = c {
                    Some(text.clone())
                } else {
                    None
                }
            })
            .unwrap_or_default();
        // Must find the match in the .ts file
        assert!(text.contains("needle"), "got: {text}");
    }

    /// Recursive walk finds files in subdirectories.
    #[tokio::test]
    async fn test_search_lines_dir_recursive() {
        let (server, tmp) = create_server().await;
        let root = tmp.path().join("root");
        std::fs::create_dir_all(root.join("deep/nested")).unwrap();
        std::fs::write(root.join("top.rs"), "// top\n").unwrap();
        std::fs::write(root.join("deep/nested/leaf.rs"), "fn target_fn() {}\n").unwrap();

        let result = server
            .handle_search_lines(
                json!({ "path": root.to_string_lossy(), "pattern": "target_fn" }),
                None,
            )
            .await
            .unwrap();

        assert!(!result.is_error.unwrap_or(false));
        let text = result
            .content
            .as_deref()
            .unwrap_or_default()
            .iter()
            .find_map(|c| {
                if let crate::mcp::types::MCPContent::Text { text, .. } = c {
                    Some(text.clone())
                } else {
                    None
                }
            })
            .unwrap_or_default();
        assert!(
            text.contains("leaf.rs"),
            "expected leaf.rs in output, got: {text}"
        );
        assert!(text.contains("target_fn"));
    }

    /// Case-insensitive flag works when searching a directory.
    #[tokio::test]
    async fn test_search_lines_dir_case_insensitive() {
        let (server, tmp) = create_server().await;
        let dir = tmp.path().join("ci");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("f.txt"), "Hello World\n").unwrap();

        // Case-sensitive: should not match lowercase
        let result_sensitive = server
            .handle_search_lines(
                json!({ "path": dir.to_string_lossy(), "pattern": "hello world", "ignoreCase": false }),
                None,
            )
            .await
            .unwrap();
        let text_s = result_sensitive
            .content
            .as_deref()
            .unwrap_or_default()
            .iter()
            .find_map(|c| {
                if let crate::mcp::types::MCPContent::Text { text, .. } = c {
                    Some(text.clone())
                } else {
                    None
                }
            })
            .unwrap_or_default();
        assert!(
            text_s.contains("No matches") || text_s.contains("no match"),
            "expected no match case-sensitive, got: {text_s}"
        );

        // Case-insensitive: must match
        let result_insensitive = server
            .handle_search_lines(
                json!({ "path": dir.to_string_lossy(), "pattern": "hello world", "ignoreCase": true }),
                None,
            )
            .await
            .unwrap();
        assert!(!result_insensitive.is_error.unwrap_or(false));
        let text_i = result_insensitive
            .content
            .as_deref()
            .unwrap_or_default()
            .iter()
            .find_map(|c| {
                if let crate::mcp::types::MCPContent::Text { text, .. } = c {
                    Some(text.clone())
                } else {
                    None
                }
            })
            .unwrap_or_default();
        assert!(
            text_i.contains("Hello World"),
            "expected match case-insensitive, got: {text_i}"
        );
    }

    /// Passing a file path still works (regression: directory branch must not break file path).
    #[tokio::test]
    async fn test_search_lines_file_path_still_works() {
        let (server, tmp) = create_server().await;
        let file = tmp.path().join("single.txt");
        std::fs::write(&file, "line one\nline two\nline three\n").unwrap();

        let result = server
            .handle_search_lines(
                json!({ "path": file.to_string_lossy(), "pattern": "line two" }),
                None,
            )
            .await
            .unwrap();

        assert!(!result.is_error.unwrap_or(false));
        let text = result
            .content
            .as_deref()
            .unwrap_or_default()
            .iter()
            .find_map(|c| {
                if let crate::mcp::types::MCPContent::Text { text, .. } = c {
                    Some(text.clone())
                } else {
                    None
                }
            })
            .unwrap_or_default();
        assert!(text.contains("line two"), "got: {text}");
    }
}
