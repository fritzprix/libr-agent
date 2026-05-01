use super::super::utils::format_file_size;
use super::helpers::*;
use crate::mcp::builtin::error_guidance::{guided_error, ErrorCategory, SuccessHint, ToolGroup};
use crate::mcp::types::MCPResult;
use serde_json::json;
use std::path::Path;

pub(super) async fn search_files_only(
    root_path: &Path,
    display_path: &str,
    pattern: &str,
    limit: usize,
    offset: usize,
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
    let mut skipped_heavy_dirs = 0usize;
    let mut skipped_gitignored_dirs = 0usize;
    let gitignore = build_gitignore_matcher(root_path);

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
        let walker = WalkDir::new(root_path)
            .into_iter()
            .filter_entry(|entry| {
                if let Some(reason) =
                    classify_search_entry_skip(root_path, entry, gitignore.as_ref())
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

    let total_matches = results.len();
    let paginated_results: Vec<_> = results.into_iter().skip(offset).take(limit).collect();
    let has_more = offset + paginated_results.len() < total_matches;

    let (result_text, next_steps) = if total_matches == 0 {
        (
            format!(
                "**🔍 File Search: No matches found**\n\n\
                Pattern: `{}`\n\
                Search Path: `{}`",
                pattern, display_path
            ),
            vec![
                "Verify the pattern syntax (use glob format like `*.txt` or `**/*.rs`)".to_string(),
                "Use listDirectory to explore available files".to_string(),
            ],
        )
    } else {
        let mut text = format!(
            "**🔍 File Search: {} file(s) found**\n\n\
            Pattern: `{}`\n\
            Search Path: `{}`\n\n",
            total_matches, pattern, display_path
        );

        text.push_str("| Type | Path | Size |\n|---|---|---|\n");

        for item in &paginated_results {
            let p = item.get("path").and_then(|v| v.as_str()).unwrap_or("?");
            let t = item.get("type").and_then(|v| v.as_str()).unwrap_or("?");
            let icon = if t == "file" { "📄 file" } else { "📁 dir" };
            if let Some(size) = item.get("size").and_then(|v| v.as_u64()) {
                text.push_str(&format!(
                    "| {} | `{}` | {} |\n",
                    icon,
                    p,
                    format_file_size(size)
                ));
            } else {
                text.push_str(&format!("| {} | `{}` | - |\n", icon, p));
            }
        }

        if has_more {
            text.push_str(&format!(
                "\n*(Showing {} to {} of {} total matches. Call search with offset: {} to see more)*\n",
                offset + 1,
                offset + paginated_results.len(),
                total_matches,
                offset + limit
            ));
        } else if offset > 0 {
            text.push_str(&format!(
                "\n*(Showing {} to {} of {} total matches)*\n",
                offset + 1,
                offset + paginated_results.len(),
                total_matches
            ));
        }

        if skipped_heavy_dirs > 0 {
            text.push_str(&format!(
                "\n*Skipped {} heavyweight director{} (`{}`)*\n",
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
        (
            text,
            vec![
                "Refine search query or filePattern if too many results were returned".to_string(),
                "Use offset and limit to paginate through results if truncated".to_string(),
                "Use search on specific directories to narrow down".to_string(),
            ],
        )
    };

    Ok(
        SuccessHint::new(result_text, next_steps).to_mcp_result_with_data(Some(json!({
            "matches": paginated_results,
            "total_matches": total_matches,
            "offset": offset,
            "limit": limit,
            "skipped_directories": skipped_heavy_dirs + skipped_gitignored_dirs,
            "skipped_heavyweight_directories": skipped_heavy_dirs,
            "skipped_gitignored_directories": skipped_gitignored_dirs,
        }))),
    )
}
