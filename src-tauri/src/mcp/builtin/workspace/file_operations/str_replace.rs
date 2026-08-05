use super::super::WorkspaceServer;
use super::utils::format_file_diff;
use crate::mcp::builtin::error_guidance::{
    guided_error, missing_param_error, not_found_error, ErrorCategory, SuccessHint, ToolGroup,
};
use crate::mcp::types::MCPResult;
use serde_json::Value;
use std::path::Path;
use tokio::fs;
use tokio::io::AsyncReadExt;

async fn read_validated_utf8_file(path: &Path) -> Result<String, String> {
    use crate::mcp::builtin::workspace::text_encoding::{decode_text_bytes, DecodedText};

    let max_size = crate::config::max_file_size();
    let metadata = fs::metadata(path)
        .await
        .map_err(|error| format!("Failed to read file metadata: {error}"))?;

    if metadata.len() > max_size as u64 {
        return Err(format!(
            "File size error: File exceeds the maximum allowed size of {max_size} bytes"
        ));
    }

    let file = fs::File::open(path)
        .await
        .map_err(|error| format!("Failed to open file: {error}"))?;

    let mut buffer = Vec::new();
    let read_limit = (max_size as u64).saturating_add(1);
    let bytes_read = file
        .take(read_limit)
        .read_to_end(&mut buffer)
        .await
        .map_err(|error| format!("Failed to read file: {error}"))?;

    if bytes_read > max_size {
        return Err(format!(
            "File size error: File exceeds the maximum allowed size of {max_size} bytes"
        ));
    }

    match decode_text_bytes(&buffer) {
        DecodedText::Binary => Err(
            "Failed to read file: content appears to be binary (embedded null bytes)".to_string(),
        ),
        DecodedText::Text { text, .. } => Ok(text),
    }
}

fn count_occurrences(content: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }

    content.match_indices(needle).count()
}

impl WorkspaceServer {
    pub async fn handle_str_replace(
        &self,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        let path_str = match args.get("path").and_then(|value| value.as_str()) {
            Some(path) if !path.trim().is_empty() => path.trim(),
            Some(_) => {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    "Path parameter cannot be empty",
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Provide a file path (relative paths resolve from the workspace)".to_string(),
                ])
                .to_mcp_result());
            }
            None => return Ok(missing_param_error("path", ToolGroup::Workspace)),
        };

        if path_str.contains("..") {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                "Path traversal patterns (..) are not allowed",
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Use a normal file path without '..' traversal segments".to_string(),
            ])
            .to_mcp_result());
        }

        let old_string = match args.get("old_string").and_then(|value| value.as_str()) {
            Some(value) if !value.is_empty() => value,
            Some(_) => {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    "Parameter 'old_string' cannot be empty",
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Provide the exact text to replace, copied from workspace__readFile output"
                        .to_string(),
                ])
                .to_mcp_result());
            }
            None => return Ok(missing_param_error("old_string", ToolGroup::Workspace)),
        };

        let new_string = match args.get("new_string").and_then(|value| value.as_str()) {
            Some(value) => value,
            None => return Ok(missing_param_error("new_string", ToolGroup::Workspace)),
        };

        let replace_all = args
            .get("replace_all")
            .and_then(|value| value.as_bool())
            .unwrap_or(false);

        let target_session_id = session_id
            .clone()
            .unwrap_or_else(|| self.session_id.clone());

        let safe_path = match self
            .validate_write_path_with_teamwork_access(path_str, Some(target_session_id))
            .await
        {
            Ok(path) => path,
            Err(error) => {
                return Ok(guided_error(
                    ErrorCategory::PermissionDenied,
                    format!("Path validation failed: {error}"),
                    ToolGroup::Workspace,
                )
                .to_mcp_result());
            }
        };

        if let Err(error) = self
            .sync_attach_before_host_read(&safe_path, session_id.as_deref())
            .await
        {
            return Ok(guided_error(
                ErrorCategory::OperationFailed,
                format!("Failed to sync attached container file before edit: {error}"),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Verify the Harbor/Docker container is still running".to_string(),
                "Retry after confirming docker exec works".to_string(),
            ])
            .to_mcp_result());
        }

        if !safe_path.exists() {
            return Ok(not_found_error("file", path_str, ToolGroup::Workspace));
        }

        let original_content = match read_validated_utf8_file(&safe_path).await {
            Ok(content) => content,
            Err(error) => {
                return Ok(
                    guided_error(ErrorCategory::InvalidInput, error, ToolGroup::Workspace)
                        .to_mcp_result(),
                );
            }
        };

        let occurrences = count_occurrences(&original_content, old_string);
        if occurrences == 0 {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                format!("old_string was not found in '{path_str}'"),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Use workspace__readFile on the target path and copy the exact text block to replace"
                    .to_string(),
                "Check whitespace, indentation, and line endings — matching is exact".to_string(),
                "For larger structural edits, split the change into smaller unique old_string values"
                    .to_string(),
            ])
            .to_mcp_result());
        }

        if !replace_all && occurrences > 1 {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                format!(
                    "old_string matched {occurrences} times in '{path_str}'. Set replace_all=true to replace every occurrence, or provide a more specific old_string."
                ),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Include more surrounding context in old_string so only one match remains".to_string(),
                "Or pass \"replace_all\": true when every occurrence should change".to_string(),
            ])
            .to_mcp_result());
        }

        let new_content = if replace_all {
            original_content.replace(old_string, new_string)
        } else {
            original_content.replacen(old_string, new_string, 1)
        };

        if new_content.len() > crate::config::max_file_size() {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                format!(
                    "Replacement would exceed the maximum allowed file size of {} bytes",
                    crate::config::max_file_size()
                ),
                ToolGroup::Workspace,
            )
            .to_mcp_result());
        }

        if let Err(error) = fs::write(&safe_path, &new_content).await {
            return Ok(guided_error(
                ErrorCategory::InternalError,
                format!("Failed to write file: {error}"),
                ToolGroup::Workspace,
            )
            .to_mcp_result());
        }

        if let Err(sync_error) = self
            .sync_attach_after_host_write(&safe_path, session_id.as_deref())
            .await
        {
            return Ok(guided_error(
                ErrorCategory::OperationFailed,
                format!(
                    "File was updated locally but failed to sync into the attached container: {sync_error}"
                ),
                ToolGroup::Workspace,
            )
            .to_mcp_result());
        }

        let replacements = if replace_all { occurrences } else { 1 };
        let diff_output = format_file_diff(&original_content, &new_content, path_str);
        let message =
            format!("Replaced {replacements} occurrence(s) in '{path_str}'.\n\n{diff_output}");

        Ok(SuccessHint::new(
            message,
            vec!["Use workspace__readFile to verify the updated content".to_string()],
        )
        .to_mcp_result())
    }
}

#[cfg(test)]
mod tests {
    use super::{count_occurrences, read_validated_utf8_file};
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn count_occurrences_handles_empty_needle() {
        assert_eq!(count_occurrences("abc", ""), 0);
    }

    #[test]
    fn count_occurrences_counts_non_overlapping_matches() {
        assert_eq!(count_occurrences("foo foo foo", "foo"), 3);
        assert_eq!(count_occurrences("aaa", "aa"), 1);
    }

    #[tokio::test]
    async fn read_validated_utf8_file_rejects_binary() {
        let mut file = NamedTempFile::new().expect("temp file");
        file.write_all(&[0xff, 0xfe, 0xfd]).expect("write bytes");
        let error = read_validated_utf8_file(file.path())
            .await
            .expect_err("binary should fail");
        assert!(error.contains("UTF-8"), "{error}");
    }
}
