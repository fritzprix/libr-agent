use super::super::WorkspaceServer;
use super::utils::{
    format_file_content_preview, format_file_size, format_preview_line, initial_prefix_hash_state,
};
use crate::mcp::builtin::error_guidance::{
    guided_error, missing_param_error, permission_denied_error, ErrorCategory, SuccessHint,
    ToolGroup,
};
use crate::mcp::builtin::workspace::edit_mode::{
    write_file_anchor_preview_note, write_file_post_write_anchor_heading, LINE_ANCHORS_ENABLED,
    PRIMARY_EDIT_TOOL,
};
use crate::mcp::types::MCPResult;
use crate::services::SecureFileManager;
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::io::BufRead;
use std::path::{Path, PathBuf};
use tracing::error;

/// Maximum numeric suffix attempts when `mode=create` hits an existing path.
const MAX_CREATE_PATH_SUFFIX: u32 = 99;

/// Soft-guard: prefer targeted edits when overwriting a file at least this many lines.
const OVERWRITE_SOFT_GUARD_MIN_LINES: usize = 40;

/// Append `-{n}` before the extension (or at the end for extensionless / bare dotfiles).
///
/// Examples: `report.md` → `report-1.md`, `.gitignore` → `.gitignore-1`, `README` → `README-1`.
fn add_numeric_suffix_to_filename(file_name: &str, n: u32) -> String {
    if file_name.is_empty() {
        return format!("file-{n}");
    }

    // Bare dotfiles (".env", ".gitignore") have no real extension — append after the name.
    if file_name.starts_with('.') && !file_name[1..].contains('.') {
        return format!("{file_name}-{n}");
    }

    match file_name.rfind('.') {
        Some(dot) if dot > 0 => {
            format!("{}-{n}.{}", &file_name[..dot], &file_name[dot + 1..])
        }
        _ => format!("{file_name}-{n}"),
    }
}

/// Replace only the final path segment of `path_str`, preserving the original separator style.
fn sibling_display_path(path_str: &str, new_file_name: &str) -> String {
    match path_str.rfind(['/', '\\']) {
        Some(idx) => format!("{}{}", &path_str[..=idx], new_file_name),
        None => new_file_name.to_string(),
    }
}

struct UniqueCreatePath {
    /// Absolute (validated) path to write.
    safe_path: PathBuf,
    /// Agent-facing path string (same style as the request).
    display_path: String,
    /// True when the requested path already existed and a suffix was allocated.
    path_adjusted: bool,
    /// Suffix number used when adjusted (e.g. 1 for `file-1.md`).
    suffix: Option<u32>,
}

/// For `mode=create`, keep the existing file and allocate `stem-N.ext` when needed.
fn allocate_unique_create_path(
    requested_path_str: &str,
    safe_path: &Path,
) -> Result<UniqueCreatePath, String> {
    if !safe_path.exists() {
        return Ok(UniqueCreatePath {
            safe_path: safe_path.to_path_buf(),
            display_path: requested_path_str.to_string(),
            path_adjusted: false,
            suffix: None,
        });
    }

    let parent = safe_path.parent().unwrap_or_else(|| Path::new("."));
    let original_name = safe_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("file");

    for n in 1..=MAX_CREATE_PATH_SUFFIX {
        let candidate_name = add_numeric_suffix_to_filename(original_name, n);
        let candidate_safe = parent.join(&candidate_name);
        if !candidate_safe.exists() {
            return Ok(UniqueCreatePath {
                safe_path: candidate_safe,
                display_path: sibling_display_path(requested_path_str, &candidate_name),
                path_adjusted: true,
                suffix: Some(n),
            });
        }
    }

    Err(format!(
        "Could not allocate a unique path for '{}': tried suffixes -1 through -{MAX_CREATE_PATH_SUFFIX} and all candidates already exist",
        requested_path_str
    ))
}

struct AppendPreview {
    total_lines: usize,
    total_size_bytes: u64,
    shown_lines: usize,
    preview_was_truncated: bool,
    display_lines: String,
}

fn read_back_append_preview(
    safe_path: &Path,
    max_display_lines: usize,
    max_display_bytes: usize,
) -> Result<AppendPreview, String> {
    let total_size_bytes = std::fs::metadata(safe_path)
        .map_err(|e| format!("failed to stat updated file: {e}"))?
        .len();
    let file = std::fs::File::open(safe_path).map_err(|e| e.to_string())?;
    let reader = std::io::BufReader::new(file);

    let mut prefix_state = initial_prefix_hash_state();
    let mut total_lines = 0usize;
    let mut preview_bytes = 0usize;
    let mut tail_lines = VecDeque::new();

    for line_result in reader.lines() {
        let line = line_result.map_err(|e| {
            if e.kind() == std::io::ErrorKind::InvalidData {
                "Content appears to be binary or contains invalid UTF-8 characters".to_string()
            } else {
                e.to_string()
            }
        })?;

        total_lines += 1;
        let preview_line = format_preview_line(total_lines, &line, &mut prefix_state);
        preview_bytes += preview_line.len() + 1;
        tail_lines.push_back(preview_line);

        while tail_lines.len() > max_display_lines
            || (preview_bytes > max_display_bytes && tail_lines.len() > 1)
        {
            if let Some(removed) = tail_lines.pop_front() {
                preview_bytes = preview_bytes.saturating_sub(removed.len() + 1);
            }
        }
    }

    let shown_lines = tail_lines.len();
    let preview_was_truncated =
        total_lines > shown_lines || total_size_bytes > max_display_bytes as u64;

    Ok(AppendPreview {
        total_lines,
        total_size_bytes,
        shown_lines,
        preview_was_truncated,
        display_lines: tail_lines.into_iter().collect::<Vec<_>>().join("\n"),
    })
}

impl WorkspaceServer {
    pub async fn handle_write_file(
        &self,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        // Layer 1: Proactive Parameter Validation

        // 1. Path parameter existence and non-empty check
        let path_str = match args.get("path").and_then(|v| v.as_str()) {
            Some(path) if !path.trim().is_empty() => path.trim(),
            Some(_) => {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    "Path parameter cannot be empty",
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Provide a file path (relative paths resolve from the workspace)".to_string(),
                    "Examples: {\"path\": \"src/main.rs\"} or {\"path\": \"/tmp/file.txt\"}"
                        .to_string(),
                    "Use workspace__listDirectory('.') to explore available paths".to_string(),
                ])
                .to_mcp_result());
            }
            None => {
                return Ok(missing_param_error("path", ToolGroup::Workspace));
            }
        };

        // 2. Path traversal validation
        if path_str.contains("..") {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                "Path traversal patterns (..) are not allowed",
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Use a normal file path without '..' traversal segments".to_string(),
                "Example: 'src/main.rs' instead of '../src/main.rs'".to_string(),
                "Use workspace__listDirectory to explore available paths".to_string(),
            ])
            .to_mcp_result());
        }

        // 3. Content parameter validation
        let content = match args.get("content").and_then(|v| v.as_str()) {
            Some(content) => content,
            None => {
                return Ok(missing_param_error("content", ToolGroup::Workspace));
            }
        };

        // 4. Determine write mode (defaults to 'create')
        let mode = args
            .get("mode")
            .and_then(|v| v.as_str())
            .unwrap_or("create");

        // Validate path security — blocks Windows reserved filenames on creation
        let target_session_id = session_id
            .clone()
            .unwrap_or_else(|| self.session_id.clone());
        let safe_path = match self
            .validate_write_path_with_teamwork_access(path_str, Some(target_session_id.clone()))
            .await
        {
            Ok(path) => path,
            Err(e) => {
                return Ok(guided_error(
                    ErrorCategory::PermissionDenied,
                    format!("Path validation failed: {}", e),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Verify the target path is not a protected location".to_string(),
                    "Use workspace__listDirectory to see available paths".to_string(),
                ])
                .to_mcp_result());
            }
        };

        if let Err(sync_error) = self
            .sync_attach_before_host_read(&safe_path, Some(target_session_id.as_str()))
            .await
        {
            return Ok(guided_error(
                ErrorCategory::OperationFailed,
                format!("Failed to sync attached container file before write: {sync_error}"),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Verify the Harbor/Docker container is still running".to_string(),
                "Retry writeFile after confirming docker exec works".to_string(),
            ])
            .to_mcp_result());
        }

        // Resolve the actual write target.
        // mode=create: if the path already exists, keep it and write to stem-N.ext instead
        // (avoids discarding already-generated content / forcing a costly retry).
        let requested_path_str = path_str.to_string();
        let requested_path_existed = safe_path.exists();
        let mut unique_create = if mode == "create" {
            match allocate_unique_create_path(path_str, &safe_path) {
                Ok(unique) => Some(unique),
                Err(e) => {
                    return Ok(guided_error(
                        ErrorCategory::DuplicateResource,
                        e,
                        ToolGroup::Workspace,
                    )
                    .guidance(vec![
                        format!(
                            "Choose a different path, or set \"mode\": \"overwrite\" to replace \"{}\".",
                            path_str
                        ),
                        format!(
                            "Use workspace__listDirectory on the parent of \"{}\" to see existing names.",
                            path_str
                        ),
                        format!(
                            "Use {} for targeted edits to \"{}\" instead of rewriting the whole file.",
                            PRIMARY_EDIT_TOOL,
                            path_str
                        ),
                    ])
                    .to_mcp_result());
                }
            }
        } else {
            None
        };

        let path_adjusted = unique_create.as_ref().is_some_and(|u| u.path_adjusted);
        let create_suffix = unique_create.as_ref().and_then(|u| u.suffix);
        let write_display_path = unique_create
            .as_ref()
            .map(|u| u.display_path.clone())
            .unwrap_or_else(|| requested_path_str.clone());

        // When path was adjusted, re-validate the sibling path for write security.
        let write_safe_path = if path_adjusted {
            match self
                .validate_write_path_with_teamwork_access(
                    &write_display_path,
                    Some(target_session_id.clone()),
                )
                .await
            {
                Ok(path) => path,
                Err(e) => {
                    return Ok(guided_error(
                        ErrorCategory::PermissionDenied,
                        format!("Adjusted path validation failed: {}", e),
                        ToolGroup::Workspace,
                    )
                    .guidance(vec![
                        format!(
                            "Could not write to auto-allocated path \"{}\". Choose a different path explicitly.",
                            write_display_path
                        ),
                        "Verify the target path is not a protected location".to_string(),
                    ])
                    .to_mcp_result());
                }
            }
        } else if let Some(unique) = unique_create.take() {
            unique.safe_path
        } else {
            safe_path.clone()
        };

        let file_exists_at_write_path = write_safe_path.exists();
        let mut old_content = String::new();

        if file_exists_at_write_path && mode == "overwrite" {
            match tokio::fs::read_to_string(&write_safe_path).await {
                Ok(c) => old_content = c,
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
                        "File exists but could not be read".to_string(),
                        "Check file permissions".to_string(),
                    ])
                    .to_mcp_result());
                }
            }
        }

        let file_manager =
            SecureFileManager::new_with_base_dir(self.get_workspace_dir(&target_session_id));
        let safe_path_str = write_safe_path.to_string_lossy().to_string();

        let result = if mode == "append" {
            file_manager
                .append_file_string(&safe_path_str, content)
                .await
        } else {
            file_manager
                .write_file_string(&safe_path_str, content)
                .await
        };

        match result {
            Ok(()) => {
                if let Err(sync_error) = self
                    .sync_attach_after_host_write(&write_safe_path, Some(&target_session_id))
                    .await
                {
                    return Ok(guided_error(
                        ErrorCategory::OperationFailed,
                        format!(
                            "File was written to staging but failed to sync into the attached container: {sync_error}"
                        ),
                        ToolGroup::Workspace,
                    )
                    .guidance(vec![
                        "Verify the Harbor/Docker container is still running".to_string(),
                        "Retry writeFile after confirming docker exec works".to_string(),
                    ])
                    .to_mcp_result());
                }

                self.invalidate_context_cache().await;

                let max_display_lines = 100;
                let max_display_bytes = 51200; // 50KB
                let append_preview = if mode == "append" {
                    let preview_path = write_safe_path.clone();
                    match tokio::task::spawn_blocking(move || {
                        read_back_append_preview(
                            &preview_path,
                            max_display_lines,
                            max_display_bytes,
                        )
                    })
                    .await
                    {
                        Ok(Ok(preview)) => Some(preview),
                        Ok(Err(error)) => {
                            return Ok(guided_error(
                                ErrorCategory::OperationFailed,
                                format!("File was updated but could not be read back: {}", error),
                                ToolGroup::Workspace,
                            )
                            .guidance(vec![
                                format!(
                                    "Use workspace__readFile(\"{}\") to inspect the current file state",
                                    write_display_path
                                ),
                                "Check file permissions if the follow-up read unexpectedly failed"
                                    .to_string(),
                            ])
                            .to_mcp_result());
                        }
                        Err(join_error) => return Err(join_error.to_string()),
                    }
                } else {
                    None
                };

                let new_lines = content.lines().count();
                let new_bytes = content.len();
                let previous_lines = if file_exists_at_write_path && mode == "overwrite" {
                    old_content.lines().count()
                } else {
                    0
                };
                let previous_bytes = if file_exists_at_write_path && mode == "overwrite" {
                    old_content.len()
                } else {
                    0
                };

                let total_lines = append_preview
                    .as_ref()
                    .map(|preview| preview.total_lines)
                    .unwrap_or(new_lines);
                let total_size_str = format_file_size(
                    append_preview
                        .as_ref()
                        .map(|preview| preview.total_size_bytes)
                        .unwrap_or(new_bytes as u64),
                );
                let appended_lines = new_lines;
                let appended_size_str = format_file_size(new_bytes as u64);

                let overwrite_diff = if file_exists_at_write_path && mode == "overwrite" {
                    Some(super::utils::compute_file_diff(
                        &old_content,
                        content,
                        &write_display_path,
                    ))
                } else {
                    None
                };

                let action = if path_adjusted {
                    "created_alternate_path"
                } else if mode == "append" {
                    "appended"
                } else if file_exists_at_write_path && mode == "overwrite" {
                    "overwritten"
                } else {
                    "created"
                };

                let message_header = if path_adjusted {
                    "**⚠️ New File Created at Alternate Path (requested path already existed)**"
                } else {
                    match (file_exists_at_write_path, mode) {
                        (true, "append") => "**✅ Content Appended Successfully**",
                        (true, "overwrite") => "**✅ File Overwritten Successfully**",
                        _ => "**✅ New File Created Successfully**",
                    }
                };

                let mut message = format!("{message_header}\n\n");

                if path_adjusted {
                    message.push_str(&format!(
                        "**What happened:** `mode` was `\"create\"` (default), but `{}` already existed.\n\
                         To avoid discarding your generated content and overwriting the existing file, \
                         the write was redirected to a new sibling path.\n\n\
                         **Requested path (unchanged):** `{}`\n\
                         **Actually written to:** `{}`\n\
                         **Total Size:** {}\n\
                         **Total Lines:** {}\n\n\
                         Subsequent reads, edits, or shell commands on `{}` still use the previous file. Use `{}` for this new content.\n\n\
                         **Correct usage reminder:**\n\
                         - To **replace** an existing file: `writeFile` with `\"mode\": \"overwrite\"`\n\
                         - To **add to the end** of an existing file: `\"mode\": \"append\"`\n\
                         - To **edit parts** of an existing file: use `{}` (not another `writeFile` create)\n\
                         - To **create a new file** when unsure the path is free: pick a unique name, or accept this auto-suffix behavior\n\n",
                        requested_path_str,
                        requested_path_str,
                        write_display_path,
                        total_size_str,
                        total_lines,
                        requested_path_str,
                        write_display_path,
                        PRIMARY_EDIT_TOOL,
                    ));
                } else {
                    message.push_str(&format!(
                        "**File:** `{}`\n**Total Size:** {}\n**Total Lines:** {}\n\n",
                        write_display_path, total_size_str, total_lines
                    ));
                }

                if mode == "append" {
                    message.push_str(&format!(
                        "**Appended:** {}, {} line(s)\n**Note:** Append is verbatim; prefix content with `\\n` when adding after an existing line.\n\n",
                        appended_size_str, appended_lines
                    ));
                }

                let mut preview_was_truncated = false;
                if let Some(diff) = overwrite_diff.as_ref() {
                    message.push_str(&diff.text);
                    if LINE_ANCHORS_ENABLED {
                        let (preview_body, truncated) = truncated_content_preview(
                            content,
                            max_display_lines,
                            max_display_bytes,
                        );
                        preview_was_truncated = truncated;
                        message.push_str(&format!(
                            "{}{}```\n{}\n```\n",
                            write_file_post_write_anchor_heading(),
                            write_file_anchor_preview_note(),
                            preview_body
                        ));
                    }
                } else if let Some(preview) = append_preview.as_ref() {
                    preview_was_truncated = preview.preview_was_truncated;
                    let display_lines = if preview.preview_was_truncated {
                        format!(
                            "{}\n\n... (truncated: showing last {} of {} lines, including the appended tail)",
                            preview.display_lines, preview.shown_lines, preview.total_lines,
                        )
                    } else {
                        preview.display_lines.clone()
                    };
                    message.push_str(write_file_anchor_preview_note());
                    message.push_str(&format!("```\n{}\n```\n", display_lines));
                } else if LINE_ANCHORS_ENABLED {
                    // create (and overwrite-of-missing): anchor preview only when needed for workspace__editFile
                    let (preview_body, truncated) =
                        truncated_content_preview(content, max_display_lines, max_display_bytes);
                    preview_was_truncated = truncated;
                    message.push_str(write_file_anchor_preview_note());
                    message.push_str(&format!("```\n{}\n```\n", preview_body));
                }

                let mut next_steps = Vec::new();
                if path_adjusted {
                    next_steps.push(format!(
                        "Written path: \"{}\" — requested \"{}\" was not modified. Continue with \"{}\" for subsequent operations.",
                        write_display_path, requested_path_str, write_display_path
                    ));
                    next_steps.push(format!(
                        "To replace \"{}\", call workspace__writeFile with \"mode\": \"overwrite\". To edit in place, use {} or mode=\"append\".",
                        requested_path_str, PRIMARY_EDIT_TOOL
                    ));
                }

                if preview_was_truncated {
                    next_steps.push(format!(
                        "Preview truncated; full file has {} line(s) at \"{}\".",
                        total_lines, write_display_path
                    ));
                }

                if file_exists_at_write_path
                    && mode == "overwrite"
                    && previous_lines >= OVERWRITE_SOFT_GUARD_MIN_LINES
                {
                    next_steps.push(format!(
                        "mode=\"overwrite\" replaced the entire file ({}+ lines); partial line edits are handled by {}, not overwrite.",
                        OVERWRITE_SOFT_GUARD_MIN_LINES, PRIMARY_EDIT_TOOL
                    ));
                }

                let hint = SuccessHint::new(message, next_steps);

                let absolute_path = write_safe_path
                    .canonicalize()
                    .unwrap_or_else(|_| write_safe_path.clone())
                    .to_string_lossy()
                    .to_string();

                let mut structured = json!({
                    "path": write_display_path,
                    "absolute_path": absolute_path,
                    "requested_path": requested_path_str,
                    "path_adjusted": path_adjusted,
                    "suffix": create_suffix,
                    "mode": mode,
                    "action": action,
                    "bytes_written": new_bytes,
                    "lines": total_lines,
                    "file_exists_before": file_exists_at_write_path,
                    "requested_path_existed": requested_path_existed
                });

                if let Some(diff) = overwrite_diff.as_ref() {
                    structured["changes"] = json!({
                        "previous_lines": previous_lines,
                        "previous_bytes": previous_bytes,
                        "new_lines": new_lines,
                        "new_bytes": new_bytes,
                        "lines_added": diff.stats.lines_added,
                        "lines_removed": diff.stats.lines_removed,
                    });
                    structured["unified_diff"] = json!(diff.text);
                }

                Ok(hint.to_mcp_result_with_data(Some(structured)))
            }
            Err(e) => {
                error!("Failed to write file {}: {}", write_display_path, e);
                let is_permission = e.to_string().contains("Permission denied")
                    || e.to_string().contains("permission");
                if is_permission {
                    Ok(permission_denied_error(
                        &write_display_path,
                        ToolGroup::Workspace,
                    ))
                } else {
                    Ok(guided_error(
                        ErrorCategory::OperationFailed,
                        e.to_string(),
                        ToolGroup::Workspace,
                    )
                    .guidance(vec![
                        "Check that the directory exists with workspace__listDirectory".to_string(),
                        "Verify you have write permissions".to_string(),
                        "Ensure the path is valid and within allowed directories".to_string(),
                    ])
                    .to_mcp_result())
                }
            }
        }
    }
}

/// Truncate in-memory content for post-write previews; returns (body, was_truncated).
fn truncated_content_preview(
    content: &str,
    max_display_lines: usize,
    max_display_bytes: usize,
) -> (String, bool) {
    let content_lines: Vec<&str> = content.lines().collect();
    let is_truncated = content_lines.len() > max_display_lines || content.len() > max_display_bytes;

    if !is_truncated {
        return (format_file_content_preview(content), false);
    }

    let truncated: Vec<&str> = if content.len() > max_display_bytes {
        let truncated_bytes = &content[..max_display_bytes.min(content.len())];
        truncated_bytes.lines().take(max_display_lines).collect()
    } else {
        content_lines
            .iter()
            .take(max_display_lines)
            .copied()
            .collect()
    };
    let partial = truncated.join("\n");
    (
        format!(
            "{}\n\n... (truncated: showing first {} of {} lines)",
            format_file_content_preview(&partial),
            truncated.len(),
            content_lines.len(),
        ),
        true,
    )
}
