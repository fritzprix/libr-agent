use path_clean::PathClean;
use std::path::{Component, Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SecurityError {
    #[error("Path traversal attempt detected: {0}")]
    PathTraversal(String),
    #[error("Access denied: {0}")]
    #[allow(dead_code)]
    AccessDenied(String),
    #[error("File size limit exceeded: {0} bytes")]
    FileSizeLimit(usize),
    #[error("Invalid path: {0}")]
    InvalidPath(String),
}

/// Security utilities for built-in servers
pub struct SecurityValidator {
    base_dir: PathBuf,
}

impl SecurityValidator {
    pub fn new_with_base_dir(base_dir: PathBuf) -> Self {
        tracing::info!(
            "SecurityValidator created with custom base_dir = {:?}",
            base_dir
        );

        // Ensure the base directory exists
        if let Err(e) = std::fs::create_dir_all(&base_dir) {
            tracing::error!("Failed to create base directory {:?}: {}", base_dir, e);
        }

        // Canonicalize base_dir so that starts_with comparisons in validate_path work
        // correctly even when base_dir itself is a symlink.
        let canonical_base = base_dir.canonicalize().unwrap_or_else(|e| {
            tracing::warn!("Failed to canonicalize base_dir {:?}: {}", base_dir, e);
            base_dir
        });

        Self {
            base_dir: canonical_base,
        }
    }

    /// Validate and clean a file path to prevent directory traversal
    pub fn validate_path(&self, user_path: &str) -> Result<PathBuf, SecurityError> {
        // Log the input and effective base directory to simplify security debugging.
        tracing::debug!(
            "Validating path: '{}' against base: '{:?}'",
            user_path,
            self.base_dir
        );

        // Normalize both Unix ('/') and Windows ('\\') style separators to '/' for consistent, cross-platform behavior.
        let normalized_path = user_path.replace(['\\', '/'], "/");
        let mut clean_path = PathBuf::from(normalized_path).clean();

        // Accept absolute paths only when they already resolve under base_dir, then convert them to a relative path.
        if clean_path.is_absolute() {
            if clean_path.starts_with(&self.base_dir) {
                match clean_path.strip_prefix(&self.base_dir) {
                    Ok(p) => {
                        clean_path = p.to_path_buf();
                        tracing::debug!("Converted absolute path to relative: {:?}", clean_path);
                    }
                    Err(e) => {
                        return Err(SecurityError::PathTraversal(format!(
                            "Failed to strip prefix from absolute path: {}",
                            e
                        )));
                    }
                }
            } else {
                return Err(SecurityError::PathTraversal(format!(
                    "Absolute paths not allowed (outside workspace): '{user_path}'"
                )));
            }
        } else {
            // Reject Windows drive-letter paths (C:, D:, ...) before joining them with the workspace base.
            if user_path.len() >= 2 && user_path.chars().nth(1) == Some(':') {
                return Err(SecurityError::PathTraversal(format!(
                    "Absolute paths with drive letters are not allowed for destination paths: '{user_path}'. \
                     Please use relative paths like 'folder/file.txt'. \
                     The file will be placed inside the workspace directory."
                )));
            }
        }

        // Block parent-directory traversal before any filesystem access.
        let traversal_check_path = user_path.replace(['\\', '/'], "/");

        if Path::new(&traversal_check_path)
            .components()
            .any(|component| matches!(component, Component::ParentDir))
        {
            return Err(SecurityError::PathTraversal(format!(
                "Parent directory traversal not allowed: '{user_path}'"
            )));
        }

        // Individual path component length limit.
        // Windows MAX_PATH is 260 chars for the full path; even with long-path
        // extensions enabled, single components > 255 chars are always invalid.
        // Reject them early to avoid triggering OS error 123 deep in the stack.
        const MAX_COMPONENT_LEN: usize = 255;
        for component in Path::new(&traversal_check_path).components() {
            if let Component::Normal(name) = component {
                if name.len() > MAX_COMPONENT_LEN {
                    return Err(SecurityError::InvalidPath(format!(
                        "Path component '{}...' exceeds maximum length of {} characters",
                        &name.to_string_lossy()[..50],
                        MAX_COMPONENT_LEN
                    )));
                }
            }
        }

        // Treat the cleaned path as relative to base_dir from this point on.
        let absolute_path = self.base_dir.join(clean_path);

        tracing::debug!("Resolved path: '{:?}'", absolute_path);

        // SecurityValidator only validates paths. Directory creation is handled explicitly by callers.

        // Canonicalize whenever possible to catch symlink-based escapes.
        let canonical_path = match absolute_path.canonicalize() {
            Ok(path) => path,
            Err(_) => {
                // The full path may not exist yet during create/write flows, so walk upward until we
                // find an existing parent. Reject unresolved symlink parents instead of skipping them,
                // because a dangling symlink can later resolve outside base_dir.
                let mut current = absolute_path.as_path();
                let mut existing_canonical = None;
                while let Some(parent) = current.parent() {
                    match parent.canonicalize() {
                        Ok(canon) => {
                            existing_canonical = Some(canon);
                            break;
                        }
                        Err(_) => {
                            if let Ok(metadata) = std::fs::symlink_metadata(parent) {
                                if metadata.file_type().is_symlink() {
                                    return Err(SecurityError::PathTraversal(format!(
                                        "Path '{}' contains an unresolved symlink parent: {:?}",
                                        user_path, parent
                                    )));
                                }
                            }
                            current = parent;
                        }
                    }
                }

                if let Some(canon) = existing_canonical {
                    if !canon.starts_with(&self.base_dir) {
                        return Err(SecurityError::PathTraversal(format!(
                            "Path '{}' resolves outside allowed directory. Base: {:?}, Resolved parent: {:?}",
                            user_path, self.base_dir, canon
                        )));
                    }
                }

                tracing::debug!(
                    "File doesn't exist yet, using non-canonical path: '{:?}'",
                    absolute_path
                );
                absolute_path.clone()
            }
        };

        // Final check: the resolved path must stay under base_dir after symlink resolution.
        if !canonical_path.starts_with(&self.base_dir) {
            return Err(SecurityError::PathTraversal(format!(
                "Path '{}' resolves outside allowed directory. Base: {:?}, Resolved: {:?}",
                user_path, self.base_dir, canonical_path
            )));
        }

        tracing::debug!("Path validation successful: '{:?}'", absolute_path);
        Ok(absolute_path)
    }

    /// Validate a path for write/create operations.
    ///
    /// Same as [`Self::validate_path`] but additionally blocks Windows reserved filenames
    /// (`CON`, `PRN`, `AUX`, `NUL`, `COM1`-`COM9`, `LPT1`-`LPT9`).  These names
    /// must be forbidden on creation because once written they become undeletable on
    /// Windows.  Deletion uses plain [`Self::validate_path`] so that pre-existing
    /// reserved-name files can still be cleaned up.
    pub fn validate_path_for_write(&self, user_path: &str) -> Result<PathBuf, SecurityError> {
        static RESERVED: &[&str] = &[
            "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
            "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
        ];
        let path_for_check = PathBuf::from(user_path.replace(['\\', '/'], "/"));
        for component in path_for_check.components() {
            if let Component::Normal(name) = component {
                // Windows strips trailing spaces and dots from path components before resolving
                // device names, so normalize those away before checking reserved names.
                let stem = Path::new(name)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .trim_end_matches([' ', '.']);
                let upper = stem.to_uppercase();
                if RESERVED.contains(&upper.as_str()) {
                    return Err(SecurityError::InvalidPath(format!(
                        "Windows reserved filename '{}' is not allowed in path: '{user_path}'",
                        stem
                    )));
                }
            }
        }
        self.validate_path(user_path)
    }

    /// Validate a path for read-only operations.
    ///
    /// Like all file operations, read paths must be strictly constrained to the base directory
    /// to prevent path traversal vulnerabilities. Absolute paths are only permitted if they
    /// resolve to a location inside the base directory.
    pub fn validate_path_for_read(&self, user_path: &str) -> Result<PathBuf, SecurityError> {
        self.validate_path(user_path)
    }

    /// Check if file size is within limits
    pub fn validate_file_size(&self, path: &Path, max_size: usize) -> Result<(), SecurityError> {
        if let Ok(metadata) = std::fs::metadata(path) {
            let file_size = metadata.len() as usize;
            if file_size > max_size {
                return Err(SecurityError::FileSizeLimit(file_size));
            }
        }
        Ok(())
    }

    /// Normalize path separators to forward slashes for cross-platform compatibility.
    /// This is useful for storing paths in databases or ZIP archives.
    pub fn normalize_path_separators(path: &str) -> String {
        path.replace(['\\', '/'], "/")
    }

    /// Extract filename from a path, supporting both / and \\ separators.
    /// Returns None if the path is empty or ends with a separator.
    pub fn extract_filename(path: &str) -> Option<String> {
        let normalized = Self::normalize_path_separators(path);
        normalized.split('/').next_back().map(|s| s.to_string())
    }
}

// ========================================
// UI Resource Response Helpers
// ========================================

use crate::mcp::types::{MCPContent, MCPResult, ServiceInfo};
use crate::repositories::session_repository::SessionRepository;
use serde_json::json;
use serde_json::Value;
use std::collections::HashSet;

/// Creates a standardized UI resource response with service information.
///
/// This helper ensures all UI resources include proper service metadata
/// for correct routing of user interactions on the frontend.
///
/// # Arguments
/// * `uri` - The resource URI (e.g., "ui://prompt/123")
/// * `mime_type` - The MIME type (typically "text/html")
/// * `html` - The rendered HTML content
/// * `server_name` - The name of the server (e.g., "ui", "playbook")
/// * `tool_name` - The name of the tool (e.g., "presentInteractive", "visualizeData")
/// * `message` - Optional text message to prepend before the resource
///
/// # Returns
/// An `MCPResult` containing the resource with embedded `ServiceInfo`
pub fn create_resource_response(
    uri: &str,
    mime_type: &str,
    html: &str,
    server_name: &str,
    tool_name: &str,
    message: Option<&str>,
) -> MCPResult {
    let service_info = ServiceInfo {
        server_name: server_name.to_string(),
        tool_name: tool_name.to_string(),
        backend_type: "BuiltInRust".to_string(),
    };

    let mut content = Vec::new();

    // Add text message if provided (for workspace tools)
    if let Some(msg) = message {
        content.push(MCPContent::Text {
            text: msg.to_string(),
            is_error: None,
        });
    }

    // Add resource content
    content.push(MCPContent::Resource {
        resource: json!({
            "uri": uri,
            "mimeType": mime_type,
            "text": html,
        }),
        service_info,
    });

    MCPResult {
        content: Some(content),
        structured_content: None,
        is_error: Some(false),
    }
}

#[derive(Debug, Clone)]
pub struct SessionToolAccess {
    pub session_id: Option<String>,
    pub allowed_builtin_aliases: Option<HashSet<String>>,
    pub allowed_external_server_ids: Option<HashSet<String>>,
    pub agent_id: Option<String>,
}

impl SessionToolAccess {
    pub fn builtin_status(&self, alias: &str) -> (&'static str, Option<String>) {
        match &self.allowed_builtin_aliases {
            Some(allowed) if allowed.contains(alias) => ("[Ready]", None),
            Some(_) => ("[Unsupported in current session]", None),
            None => ("[Ready]", None),
        }
    }

    pub fn external_status(
        &self,
        server_id: &str,
        server_name: &str,
    ) -> (&'static str, Option<String>) {
        match &self.allowed_external_server_ids {
            Some(allowed) if allowed.contains(server_id) => ("[Ready]", None),
            Some(_) => ("[Unsupported in current session]", None),
            None => {
                let _ = (server_name, server_id);
                ("[Unsupported in current session]", None)
            }
        }
    }
}

pub async fn load_session_tool_access(session_id: Option<&str>) -> SessionToolAccess {
    let Some(session_id) = session_id else {
        return SessionToolAccess {
            session_id: None,
            allowed_builtin_aliases: None,
            allowed_external_server_ids: None,
            agent_id: None,
        };
    };

    let repo = crate::state::get_session_repository();
    let session = match repo.get_session(session_id).await {
        Ok(Some(session)) => session,
        _ => {
            return SessionToolAccess {
                session_id: Some(session_id.to_string()),
                allowed_builtin_aliases: None,
                allowed_external_server_ids: None,
                agent_id: None,
            };
        }
    };

    let Some(config_str) = session.agent_config else {
        return SessionToolAccess {
            session_id: Some(session_id.to_string()),
            allowed_builtin_aliases: None,
            allowed_external_server_ids: None,
            agent_id: None,
        };
    };

    let agent_config = match crate::agent::AgentConfig::from_json(&config_str) {
        Ok(config) => config,
        Err(_) => {
            return SessionToolAccess {
                session_id: Some(session_id.to_string()),
                allowed_builtin_aliases: None,
                allowed_external_server_ids: None,
                agent_id: None,
            };
        }
    };

    let agent_id = agent_config.id.clone().or_else(|| {
        serde_json::from_str::<Value>(&config_str)
            .ok()
            .and_then(|value| {
                value
                    .get("assistantId")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
    });

    let allowed_builtin_aliases = Some(
        crate::agent::tools::runtime_allowed_builtin_service_aliases(&agent_config)
            .into_iter()
            .collect::<HashSet<_>>(),
    );

    let allowed_external_server_ids = Some(
        agent_config
            .mcp_server_ids
            .into_iter()
            .collect::<HashSet<_>>(),
    );

    SessionToolAccess {
        session_id: Some(session_id.to_string()),
        allowed_builtin_aliases,
        allowed_external_server_ids,
        agent_id,
    }
}
