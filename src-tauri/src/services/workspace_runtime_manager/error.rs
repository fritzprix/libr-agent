use thiserror::Error;

pub(super) type RuntimeResult<T> = Result<T, WorkspaceRuntimeError>;

#[derive(Debug, Error)]
pub enum WorkspaceRuntimeError {
    #[error("Docker CLI is not installed or not available on PATH. Details: {0}")]
    DockerNotInstalled(String),
    #[error("Docker is not available. Ensure Docker Desktop/daemon is running and the current user can run Docker without sudo. Details: {0}")]
    DockerNotAvailable(String),
    #[error("Docker command failed: {0}")]
    DockerCommandFailed(String),
    #[error("Docker container '{container_name}' is not owned by session '{session_id}'")]
    OwnershipMismatch {
        container_name: String,
        session_id: String,
    },
    #[error("dockerConfig is required for Docker workspace isolation")]
    MissingDockerConfig,
    #[error("Missing Docker container name for session {0}")]
    MissingContainerName(String),
    #[error("Missing Docker host workspace path for session {0}")]
    MissingHostWorkspacePath(String),
    #[error("Invalid Docker workspace config: {0}")]
    InvalidConfig(String),
    #[error("Docker workspace path is not valid UTF-8: {0}")]
    InvalidWorkspacePath(String),
    #[error("Docker image for session {session_id} must include bash for shell execution. Details: {details}")]
    BashUnavailable { session_id: String, details: String },
    #[error(
        "Docker image for session {session_id} must include bash or POSIX sh for shell execution"
    )]
    ShellUnavailable { session_id: String },
    #[error("Docker host port {0} is already in use on 127.0.0.1")]
    HostPortUnavailable(u16),
    #[error("{0}")]
    Io(String),
}

/// Prefix for structured errors surfaced to the frontend agent layer.
pub const AGENT_ERROR_DOCKER_NOT_AVAILABLE: &str = "DOCKER_NOT_AVAILABLE:";
/// Prefix when the Docker CLI binary itself is missing from PATH.
pub const AGENT_ERROR_DOCKER_NOT_INSTALLED: &str = "DOCKER_NOT_INSTALLED:";

impl WorkspaceRuntimeError {
    /// Converts runtime errors into agent-facing strings with stable machine-readable codes.
    pub fn to_agent_string(self) -> String {
        match self {
            Self::DockerNotInstalled(_) => {
                format!("{} {}", AGENT_ERROR_DOCKER_NOT_INSTALLED, self)
            }
            Self::DockerNotAvailable(_) => {
                format!("{} {}", AGENT_ERROR_DOCKER_NOT_AVAILABLE, self)
            }
            other => other.to_string(),
        }
    }
}

pub(super) fn map_docker_spawn_error(error: std::io::Error) -> WorkspaceRuntimeError {
    if error.kind() == std::io::ErrorKind::NotFound {
        WorkspaceRuntimeError::DockerNotInstalled(format!(
            "Docker CLI is not installed or not available on PATH: {error}"
        ))
    } else {
        WorkspaceRuntimeError::DockerCommandFailed(error.to_string())
    }
}

pub(super) fn as_docker_unavailable(error: WorkspaceRuntimeError) -> WorkspaceRuntimeError {
    match error {
        WorkspaceRuntimeError::DockerNotInstalled(_)
        | WorkspaceRuntimeError::DockerNotAvailable(_) => error,
        other => WorkspaceRuntimeError::DockerNotAvailable(other.to_string()),
    }
}

pub(super) fn format_docker_failure(output: &std::process::Output) -> WorkspaceRuntimeError {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let details = if !stderr.trim().is_empty() {
        stderr.trim()
    } else {
        stdout.trim()
    };
    WorkspaceRuntimeError::DockerCommandFailed(format!(
        "docker exited with status {}: {details}",
        output.status
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_not_found_spawn_error_to_docker_not_installed() {
        let error = std::io::Error::new(std::io::ErrorKind::NotFound, "program not found");
        match map_docker_spawn_error(error) {
            WorkspaceRuntimeError::DockerNotInstalled(details) => {
                assert!(details.contains("not available on PATH"));
                assert!(details.contains("program not found"));
            }
            other => panic!("expected DockerNotInstalled, got {other}"),
        }
    }

    #[test]
    fn maps_other_spawn_errors_to_docker_command_failed() {
        let error = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
        match map_docker_spawn_error(error) {
            WorkspaceRuntimeError::DockerCommandFailed(details) => {
                assert!(details.contains("denied"));
            }
            other => panic!("expected DockerCommandFailed, got {other}"),
        }
    }

    #[test]
    fn healthcheck_preserves_not_installed_classification() {
        let error = WorkspaceRuntimeError::DockerNotInstalled("missing cli".to_string());
        match as_docker_unavailable(error) {
            WorkspaceRuntimeError::DockerNotInstalled(details) => {
                assert_eq!(details, "missing cli");
            }
            other => panic!("expected DockerNotInstalled, got {other}"),
        }
    }

    #[test]
    fn healthcheck_maps_command_failures_to_not_available() {
        let error = WorkspaceRuntimeError::DockerCommandFailed("daemon down".to_string());
        match as_docker_unavailable(error) {
            WorkspaceRuntimeError::DockerNotAvailable(details) => {
                assert!(details.contains("daemon down"));
            }
            other => panic!("expected DockerNotAvailable, got {other}"),
        }
    }

    #[test]
    fn docker_not_installed_uses_structured_agent_prefix() {
        let agent =
            WorkspaceRuntimeError::DockerNotInstalled("missing".to_string()).to_agent_string();
        assert!(agent.starts_with(AGENT_ERROR_DOCKER_NOT_INSTALLED));
        assert!(agent.contains("Docker CLI is not installed"));
        assert!(!agent.starts_with(AGENT_ERROR_DOCKER_NOT_AVAILABLE));
    }

    #[test]
    fn docker_not_available_keeps_existing_prefix() {
        let agent =
            WorkspaceRuntimeError::DockerNotAvailable("daemon down".to_string()).to_agent_string();
        assert!(agent.starts_with(AGENT_ERROR_DOCKER_NOT_AVAILABLE));
        assert!(agent.contains("Docker is not available"));
    }

    #[test]
    fn docker_command_failed_has_no_availability_prefix() {
        let agent =
            WorkspaceRuntimeError::DockerCommandFailed("boom".to_string()).to_agent_string();
        assert!(!agent.contains(AGENT_ERROR_DOCKER_NOT_AVAILABLE));
        assert!(!agent.contains(AGENT_ERROR_DOCKER_NOT_INSTALLED));
        assert!(agent.contains("boom"));
    }
}
