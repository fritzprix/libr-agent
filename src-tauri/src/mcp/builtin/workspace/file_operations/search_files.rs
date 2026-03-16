use super::super::WorkspaceServer;
use super::utils::format_file_size;
use crate::mcp::builtin::error_guidance::{
    guided_error, missing_param_error, ErrorCategory, ToolGroup,
};
use crate::mcp::types::MCPResult;
use serde_json::{json, Value};
use tracing::error;

impl WorkspaceServer {
    pub async fn handle_search_files(
        &self,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        let pattern = match args.get("pattern").and_then(|v| v.as_str()) {
            Some(pattern) => pattern,
            None => {
                return Ok(missing_param_error("pattern", ToolGroup::Workspace));
            }
        };

        let search_path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
        let max_depth = args
            .get("max_depth")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize);
        let file_type = args
            .get("file_type")
            .and_then(|v| v.as_str())
            .unwrap_or("both");

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
                    "Verify the directory path is correct".to_string(),
                    "Use listDirectory to see available files".to_string(),
                ])
                .to_mcp_result());
            }
        };

        match self
            .search_files_by_pattern(&safe_path, pattern, max_depth, file_type)
            .await
        {
            Ok(results) => {
                let result_text = if results.is_empty() {
                    format!(
                        "**🔍 File Search: No matches found**\n\n\
                        Pattern: `{}`\n\
                        Search Path: `{}`\n\
                        File Type: {}\n\n\
                        **Next Steps:**\n\
                        - Verify the pattern syntax (use glob format like `*.txt` or `**/*.rs`)\n\
                        - Use listDirectory to explore available files\n\
                        - Try a broader pattern or different search path",
                        pattern, search_path, file_type
                    )
                } else {
                    let mut text = format!(
                        "**🔍 File Search: {} file(s) found**\n\n\
                        Pattern: `{}`\n\
                        Search Path: `{}`\n\
                        File Type: {}\n\n",
                        results.len(),
                        pattern,
                        search_path,
                        file_type
                    );

                    // Get metadata for file sizes
                    let mut enriched_results = Vec::new();
                    for item in results.iter().take(50) {
                        let path_str = item.get("path").and_then(|v| v.as_str()).unwrap_or("?");
                        let type_ = item.get("type").and_then(|v| v.as_str()).unwrap_or("?");

                        let size_str = if type_ == "file" {
                            let full_path = safe_path.join(path_str);
                            match tokio::fs::metadata(&full_path).await {
                                Ok(metadata) => format!(" ({})", format_file_size(metadata.len())),
                                Err(_) => String::new(),
                            }
                        } else {
                            String::new()
                        };

                        let icon = if type_ == "file" { "📄" } else { "📁" };
                        enriched_results.push((icon, path_str, size_str));
                    }

                    // Display results
                    text.push_str("**Matches:**\n");
                    for (icon, path, size) in &enriched_results {
                        text.push_str(&format!("- {} `{}`{}\n", icon, path, size));
                    }

                    if results.len() > 50 {
                        text.push_str(&format!(
                            "\n*Showing first 50 of {} total matches*\n",
                            results.len()
                        ));
                    }

                    text.push_str(
                        "\n**Next Steps:**\n\
                        - Use readFile to view file contents\n\
                        - Use searchLines to search within matching files\n\
                        - Refine pattern for more specific results",
                    );

                    text
                };

                Ok(MCPResult::success_with_data(
                    &result_text,
                    json!({ "matches": results }),
                ))
            }
            Err(e) => {
                error!("File search failed: {}", e);
                Ok(guided_error(
                    ErrorCategory::OperationFailed,
                    e.to_string(),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Verify the pattern syntax is correct (use glob format like '*.txt' or '**/*.rs')".to_string(),
                    "Check if the directory path exists with listDirectory".to_string(),
                    "Try a simpler pattern to narrow down the issue".to_string(),
                ])
                .to_mcp_result())
            }
        }
    }

    async fn search_files_by_pattern(
        &self,
        root_path: &std::path::Path,
        pattern: &str,
        max_depth: Option<usize>,
        file_type: &str,
    ) -> Result<Vec<serde_json::Value>, String> {
        use glob::Pattern;
        use walkdir::WalkDir;

        let glob_pattern = Pattern::new(pattern).map_err(|e| format!("Invalid pattern: {e}"))?;
        let mut results = Vec::new();

        let walker = if let Some(depth) = max_depth {
            WalkDir::new(root_path).max_depth(depth)
        } else {
            WalkDir::new(root_path)
        };

        for entry in walker {
            let entry = entry.map_err(|e| format!("Walk error: {e}"))?;
            let path = entry.path();

            let is_dir = path.is_dir();
            let is_file = path.is_file();

            let should_include = match file_type {
                "file" => is_file,
                "dir" => is_dir,
                "both" => is_file || is_dir,
                _ => is_file || is_dir,
            };

            if !should_include {
                continue;
            }

            let file_name = path.file_name().and_then(|n| n.to_str());

            // Get relative path for matching
            let relative_path = path.strip_prefix(root_path).unwrap_or(path);

            if matches_glob(&glob_pattern, relative_path, file_name) {
                let metadata = entry
                    .metadata()
                    .map_err(|e| format!("Metadata error: {e}"))?;

                let path_str = {
                    let p = relative_path.to_string_lossy().to_string();
                    #[cfg(target_os = "windows")]
                    let p = p.replace('\\', "/");
                    p
                };

                results.push(json!({
                    "path": path_str,
                    "name": file_name.unwrap_or(""),
                    "type": if is_dir { "directory" } else { "file" },
                    "size": if is_file { Some(metadata.len()) } else { None }
                }));
            }
        }

        Ok(results)
    }
}

/// Helper function to match paths against glob patterns in a cross-platform way.
/// Normalizes Windows backslashes to forward slashes before matching.
fn matches_glob(pattern: &glob::Pattern, path: &std::path::Path, file_name: Option<&str>) -> bool {
    // 1. Try matching the file name directly (common case)
    if let Some(name) = file_name {
        if pattern.matches(name) {
            return true;
        }
    }

    let path_str = path.to_string_lossy();

    // 2. Try matching the path as-is (Unix default)
    if pattern.matches(&path_str) {
        return true;
    }

    // 3. Try matching normalized path (Windows compatibility)
    // If the path contains backslashes, normalize to forward slashes because
    // glob patterns standardly use forward slashes.
    if path_str.contains('\\') {
        let normalized = path_str.replace('\\', "/");
        if pattern.matches(&normalized) {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use glob::Pattern;
    use std::path::PathBuf;

    #[test]
    fn test_matches_glob_unix() {
        let pattern = Pattern::new("src/**/*.rs").unwrap();

        let path = PathBuf::from("src/main.rs");
        assert!(matches_glob(
            &pattern,
            &path,
            path.file_name().and_then(|n| n.to_str())
        ));

        let path = PathBuf::from("src/subdir/test.rs");
        assert!(matches_glob(
            &pattern,
            &path,
            path.file_name().and_then(|n| n.to_str())
        ));

        let path = PathBuf::from("other/file.rs");
        assert!(!matches_glob(
            &pattern,
            &path,
            path.file_name().and_then(|n| n.to_str())
        ));
    }

    #[test]
    fn test_matches_glob_windows() {
        let pattern = Pattern::new("src/**/*.rs").unwrap();

        // Construct Windows-style path manually
        let path_str = "src\\main.rs";
        let path = PathBuf::from(path_str);

        // Note: On Unix, PathBuf::from("src\\main.rs") treats it as a single filename "src\main.rs"
        // so to test this properly on Unix runner, we need to ensure the path string has backslashes
        // and matches_glob handles it.
        // matches_glob uses to_string_lossy(), which preserves the backslashes if constructed from string.

        assert!(matches_glob(&pattern, &path, None));

        let path_str = "src\\subdir\\test.rs";
        let path = PathBuf::from(path_str);
        assert!(matches_glob(&pattern, &path, None));
    }

    #[test]
    fn test_matches_glob_filename() {
        let pattern = Pattern::new("*.txt").unwrap();
        let path = PathBuf::from("path/to/file.txt");
        assert!(matches_glob(&pattern, &path, Some("file.txt")));
    }
}
