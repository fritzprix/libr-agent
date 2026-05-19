use glob::Pattern;
use once_cell::sync::Lazy;
use path_clean::PathClean;
use regex::Regex;
use serde::Deserialize;
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
    enforce_base_dir_containment: bool,
}

impl SecurityValidator {
    pub fn new_with_base_dir(base_dir: PathBuf) -> Self {
        Self::new_internal(base_dir, false)
    }

    pub fn new_scoped_with_base_dir(base_dir: PathBuf) -> Self {
        Self::new_internal(base_dir, true)
    }

    fn new_internal(base_dir: PathBuf, enforce_base_dir_containment: bool) -> Self {
        tracing::info!(
            "SecurityValidator created with custom base_dir = {:?}, scoped = {}",
            base_dir,
            enforce_base_dir_containment
        );

        if let Err(e) = std::fs::create_dir_all(&base_dir) {
            tracing::error!("Failed to create base directory {:?}: {}", base_dir, e);
        }

        let canonical_base = base_dir.canonicalize().unwrap_or_else(|e| {
            tracing::warn!("Failed to canonicalize base_dir {:?}: {}", base_dir, e);
            base_dir
        });

        Self {
            base_dir: canonical_base,
            enforce_base_dir_containment,
        }
    }

    /// Validate and clean a file path to prevent directory traversal.
    ///
    /// Read-only validation intentionally allows general absolute paths outside
    /// `base_dir` as long as they are not sensitive and do not traverse via
    /// parent segments or unsafe symlinks. Write validation tightens this by
    /// requiring the resolved target to stay within `base_dir`.
    pub fn validate_path(&self, user_path: &str) -> Result<PathBuf, SecurityError> {
        self.validate_path_internal(user_path, self.enforce_base_dir_containment)
    }

    fn validate_path_internal(
        &self,
        user_path: &str,
        enforce_base_containment: bool,
    ) -> Result<PathBuf, SecurityError> {
        // Log the input and effective base directory to simplify security debugging.
        tracing::debug!(
            "Validating path: '{}' against base: '{:?}'",
            user_path,
            self.base_dir
        );

        // Normalize both Unix ('/') and Windows ('\\') style separators to '/' for consistent, cross-platform behavior.
        let normalized_path = user_path.replace(['\\', '/'], "/");
        let clean_path = PathBuf::from(normalized_path).clean();

        let absolute_path = if clean_path.is_absolute() {
            clean_path
        } else if cfg!(windows)
            && user_path.len() >= 2
            && user_path.as_bytes()[0].is_ascii_alphabetic()
            && user_path.as_bytes()[1] == b':'
        {
            PathBuf::from(user_path)
        } else {
            self.base_dir.join(clean_path)
        };

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

        tracing::debug!("Resolved path: '{:?}'", absolute_path);

        if enforce_base_containment && !absolute_path.starts_with(&self.base_dir) {
            return Err(SecurityError::PathTraversal(format!(
                "Access denied: Path '{user_path}' is outside the allowed base directory"
            )));
        }

        self.ensure_not_sensitive_path(&absolute_path, user_path)?;

        // SecurityValidator only validates paths. Directory creation is handled explicitly by callers.

        // Canonicalize whenever possible to catch symlink-based escapes.
        let canonical_path = match absolute_path.canonicalize() {
            Ok(path) => path,
            Err(_) => {
                // If canonicalize fails, it might be because the target doesn't exist yet.
                // However, check if the exact target path itself is an unresolved symlink.
                // If it is, we must reject it to prevent arbitrary file writes via dangling symlinks.
                if let Ok(metadata) = std::fs::symlink_metadata(&absolute_path) {
                    if metadata.file_type().is_symlink() {
                        return Err(SecurityError::PathTraversal(format!(
                            "Path '{}' is an unresolved symlink: {:?}",
                            user_path, absolute_path
                        )));
                    }
                }

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
                    if enforce_base_containment && !canon.starts_with(&self.base_dir) {
                        return Err(SecurityError::PathTraversal(format!(
                            "Access denied: Path '{user_path}' resolves outside the allowed base directory"
                        )));
                    }
                    self.ensure_not_sensitive_path(&canon, user_path)?;
                }

                tracing::debug!(
                    "File doesn't exist yet, using non-canonical path: '{:?}'",
                    absolute_path
                );
                absolute_path.clone()
            }
        };

        if enforce_base_containment && !canonical_path.starts_with(&self.base_dir) {
            return Err(SecurityError::PathTraversal(format!(
                "Access denied: Path '{user_path}' resolves outside the allowed base directory"
            )));
        }

        self.ensure_not_sensitive_path(&canonical_path, user_path)?;

        tracing::debug!("Path validation successful: '{:?}'", absolute_path);
        Ok(absolute_path)
    }

    fn ensure_not_sensitive_path(
        &self,
        candidate_path: &Path,
        user_path: &str,
    ) -> Result<(), SecurityError> {
        if self.is_sensitive_path(candidate_path) {
            return Err(SecurityError::AccessDenied(format!(
                "Path '{}' targets a protected location: '{}'",
                user_path,
                candidate_path.display()
            )));
        }

        Ok(())
    }

    fn is_sensitive_path(&self, candidate_path: &Path) -> bool {
        SENSITIVE_PATH_POLICY.matches(candidate_path)
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
        self.validate_path_internal(user_path, self.enforce_base_dir_containment)
    }

    /// Validate a path for read-only operations.
    ///
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

static SENSITIVE_PATH_POLICY: Lazy<SensitivePathPolicy> = Lazy::new(load_sensitive_path_policy);

pub fn matches_sensitive_path_policy(candidate_path: &Path) -> bool {
    SENSITIVE_PATH_POLICY.matches(candidate_path)
}

#[derive(Debug, Deserialize)]
struct SensitivePathPolicyConfig {
    rules: Vec<SensitivePathRuleConfig>,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SensitivePathRuleScope {
    Absolute,
    Home,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SensitivePathRuleMatch {
    Exact,
    Glob,
    Pattern,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum SensitivePathRuleOs {
    Linux,
    Macos,
    Windows,
}

#[derive(Debug, Deserialize)]
struct SensitivePathRuleConfig {
    scope: SensitivePathRuleScope,
    #[serde(rename = "match")]
    matcher: SensitivePathRuleMatch,
    value: String,
    #[serde(default)]
    os: Vec<SensitivePathRuleOs>,
}

#[derive(Debug)]
struct SensitivePathPolicy {
    rules: Vec<CompiledSensitivePathRule>,
}

#[derive(Debug)]
struct CompiledSensitivePathRule {
    scope: SensitivePathRuleScope,
    matcher: CompiledSensitivePathRuleMatcher,
    os: Vec<SensitivePathRuleOs>,
}

#[derive(Debug)]
enum CompiledSensitivePathRuleMatcher {
    Exact(String),
    Glob(Pattern),
    Pattern(Regex),
}

impl SensitivePathPolicy {
    fn matches(&self, candidate_path: &Path) -> bool {
        self.rules.iter().any(|rule| rule.matches(candidate_path))
    }
}

impl CompiledSensitivePathRule {
    fn matches(&self, candidate_path: &Path) -> bool {
        if !self.os.is_empty() && !self.os.contains(&current_policy_os()) {
            return false;
        }

        let target = match self.scope {
            SensitivePathRuleScope::Absolute => normalize_policy_path(candidate_path),
            SensitivePathRuleScope::Home => {
                let Some(relative_path) = relative_path_under_any_user_home(candidate_path) else {
                    return false;
                };
                normalize_policy_value(&relative_path)
            }
        };

        match &self.matcher {
            CompiledSensitivePathRuleMatcher::Exact(value) => target == *value,
            CompiledSensitivePathRuleMatcher::Glob(pattern) => pattern.matches(&target),
            CompiledSensitivePathRuleMatcher::Pattern(regex) => regex.is_match(&target),
        }
    }
}

fn load_sensitive_path_policy() -> SensitivePathPolicy {
    let config: SensitivePathPolicyConfig =
        serde_json::from_str(include_str!("sensitive_path_policy.json"))
            .expect("sensitive path policy JSON must be valid");

    let rules = config
        .rules
        .into_iter()
        .map(|rule| {
            let matcher =
                compile_sensitive_path_matcher(rule.matcher, &rule.value).unwrap_or_else(|error| {
                    panic!(
                        "Invalid sensitive path policy rule ({:?} {:?} {:?}): {}",
                        rule.scope, rule.matcher, rule.value, error
                    )
                });

            CompiledSensitivePathRule {
                scope: rule.scope,
                matcher,
                os: rule.os,
            }
        })
        .collect();

    SensitivePathPolicy { rules }
}

fn compile_sensitive_path_matcher(
    matcher: SensitivePathRuleMatch,
    value: &str,
) -> Result<CompiledSensitivePathRuleMatcher, String> {
    let normalized_value = normalize_policy_value(value);

    match matcher {
        SensitivePathRuleMatch::Exact => {
            Ok(CompiledSensitivePathRuleMatcher::Exact(normalized_value))
        }
        SensitivePathRuleMatch::Glob => Pattern::new(&normalized_value)
            .map(CompiledSensitivePathRuleMatcher::Glob)
            .map_err(|error| error.msg.to_string()),
        SensitivePathRuleMatch::Pattern => Regex::new(value)
            .map(CompiledSensitivePathRuleMatcher::Pattern)
            .map_err(|error| error.to_string()),
    }
}

fn normalize_policy_path(path: &Path) -> String {
    normalize_policy_value(&path.to_string_lossy())
}

fn normalize_policy_value(value: &str) -> String {
    let normalized =
        normalize_macos_private_aliases(&SecurityValidator::normalize_path_separators(value));
    if cfg!(windows) {
        normalized.to_ascii_lowercase()
    } else {
        normalized
    }
}

fn normalize_macos_private_aliases(value: &str) -> String {
    #[cfg(target_os = "macos")]
    {
        for public_root in ["/etc", "/var", "/tmp"] {
            let private_root = format!("/private{public_root}");
            if value == private_root {
                return public_root.to_string();
            }
            if let Some(suffix) = value.strip_prefix(&(private_root.clone() + "/")) {
                return format!("{public_root}/{suffix}");
            }
        }
    }

    value.to_string()
}

fn current_policy_os() -> SensitivePathRuleOs {
    #[cfg(target_os = "linux")]
    {
        SensitivePathRuleOs::Linux
    }
    #[cfg(target_os = "macos")]
    {
        SensitivePathRuleOs::Macos
    }
    #[cfg(target_os = "windows")]
    {
        SensitivePathRuleOs::Windows
    }
}

fn component_eq(component: &Component<'_>, expected: &str) -> bool {
    matches!(component, Component::Normal(name) if normalize_policy_value(&name.to_string_lossy()) == normalize_policy_value(expected))
}

fn relative_path_under_any_user_home(path: &Path) -> Option<String> {
    if let Some(current_home) = dirs::home_dir() {
        if let Ok(stripped) = path.strip_prefix(&current_home) {
            return Some(join_normal_components(stripped.components()));
        }
    }

    let components: Vec<_> = path.components().collect();

    #[cfg(unix)]
    {
        match components.as_slice() {
            [Component::RootDir, user_home, rest @ ..] if component_eq(user_home, "root") => {
                return Some(join_normal_component_slice(rest));
            }
            [Component::RootDir, home_root, Component::Normal(_), rest @ ..]
                if component_eq(home_root, "home") || component_eq(home_root, "Users") =>
            {
                return Some(join_normal_component_slice(rest));
            }
            _ => {}
        }
    }

    #[cfg(windows)]
    {
        match components.as_slice() {
            [Component::Prefix(_), Component::RootDir, users_root, Component::Normal(_), rest @ ..]
                if component_eq(users_root, "Users")
                    || component_eq(users_root, "Documents and Settings") =>
            {
                return Some(join_normal_component_slice(rest));
            }
            _ => {}
        }
    }

    None
}

fn join_normal_components<'a>(components: impl Iterator<Item = Component<'a>>) -> String {
    components
        .filter_map(|component| match component {
            Component::Normal(name) => Some(name.to_string_lossy().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn join_normal_component_slice(components: &[Component<'_>]) -> String {
    components
        .iter()
        .filter_map(|component| match component {
            Component::Normal(name) => Some(name.to_string_lossy().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
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
