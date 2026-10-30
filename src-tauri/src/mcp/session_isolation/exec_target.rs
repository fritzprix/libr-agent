//! Where the stdio MCP servers of a session run.
//!
//! In a Docker session the workspace is the container: `runShell` and stdio MCP servers both
//! run there. Host sessions run stdio servers on the host.

use super::channel_transport::ChildExitReport;
use crate::models::workspace_isolation::WorkspaceIsolationMode;
use crate::repositories::{SessionMetadata, SessionRepository};

#[derive(Debug, Clone)]
pub enum StdioExecTarget {
    Host,
    Container(Box<SessionMetadata>),
}

impl StdioExecTarget {
    /// Resolves the execution location from the session record. A Docker session must not
    /// fall back to the host, so a database error fails instead of returning `Host`.
    pub async fn for_session(session_id: Option<&str>) -> Result<Self, String> {
        let Some(session_id) = session_id else {
            return Ok(Self::Host);
        };
        let Some(session_repo) = crate::state::try_get_session_repository() else {
            return Ok(Self::Host);
        };
        match session_repo.get_session(session_id).await {
            Ok(Some(session)) => Ok(Self::from_session(session)),
            Ok(None) => Ok(Self::Host),
            Err(error) => Err(format!(
                "Failed to read workspace isolation of session '{session_id}': {error}"
            )),
        }
    }

    pub fn from_session(session: SessionMetadata) -> Self {
        if session.workspace_isolation == WorkspaceIsolationMode::Docker {
            Self::Container(Box::new(session))
        } else {
            Self::Host
        }
    }

    pub fn container_session(&self) -> Option<&SessionMetadata> {
        match self {
            Self::Host => None,
            Self::Container(session) => Some(session),
        }
    }
}

/// The image (or attached container) of a Docker session, as shown to the session.
pub fn session_image(session: &SessionMetadata) -> String {
    let config = session.docker_config.as_ref();
    if let Some(image) = config.and_then(|config| config.image_ref()) {
        return image.to_string();
    }
    match config.and_then(|config| config.attach_container_name()) {
        Some(container) => format!("the image of attached container '{container}'"),
        None => "unknown".to_string(),
    }
}

/// True when `docker exec` could not start the command: exit 127 (not found), exit 126 (not
/// executable), or the OCI runtime reported that the exec failed.
pub fn is_runtime_missing(report: &ChildExitReport) -> bool {
    if matches!(report.exit_code(), Some(126 | 127)) {
        return true;
    }
    report.stderr_tail().iter().any(|line| {
        let line = line.to_ascii_lowercase();
        line.contains("oci runtime exec failed") || line.contains("executable file not found")
    })
}

pub fn runtime_missing_message(command: &str, image: &str) -> String {
    format!(
        "The command '{command}' is not in the session container. \
         The image of this session is '{image}'. \
         In a Docker session, stdio MCP servers run in the session container, not on the host. \
         A server command with a host path does not resolve in the container. \
         Install the runtime in the container (for example with workspace__runShell), \
         then register or verify the server again."
    )
}

#[cfg(test)]
pub(crate) fn test_docker_session(
    config: crate::models::workspace_isolation::DockerWorkspaceConfig,
) -> SessionMetadata {
    SessionMetadata {
        id: "docker-session".to_string(),
        name: None,
        status: crate::repositories::SessionStatus::Idle,
        model: "model".to_string(),
        provider: "provider".to_string(),
        assistant_id: None,
        parent_session_id: None,
        lineage_id: None,
        depth: None,
        max_depth: None,
        max_fanout: None,
        org_id: None,
        org_name: None,
        org_root_session_id: None,
        created_at: 0,
        updated_at: 0,
        last_viewed_at: None,
        last_message_at: None,
        last_attention_at: None,
        last_attention_reason: None,
        is_bookmarked: false,
        execution_mode: crate::execution_mode::ExecutionMode::Normal,
        workspace_override: None,
        workspace_isolation: WorkspaceIsolationMode::Docker,
        docker_config: Some(config),
        docker_container_name: None,
        docker_host_workspace_path: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::workspace_isolation::DockerWorkspaceConfig;

    fn report(exit_code: Option<i32>, stderr: &[&str]) -> ChildExitReport {
        let report = ChildExitReport::default();
        report.record_exit_code(exit_code);
        for line in stderr {
            report.push_stderr(line);
        }
        report
    }

    fn docker_config(image: Option<&str>, attach: Option<&str>) -> DockerWorkspaceConfig {
        DockerWorkspaceConfig {
            image: image.map(str::to_string),
            attach_container: attach.map(str::to_string),
            workdir: None,
            manage_lifecycle: None,
            env: Default::default(),
            port_bindings: Vec::new(),
        }
    }

    #[test]
    fn exit_127_and_126_mean_the_runtime_is_missing() {
        assert!(is_runtime_missing(&report(Some(127), &[])));
        assert!(is_runtime_missing(&report(Some(126), &[])));
    }

    #[test]
    fn oci_enoent_on_stderr_means_the_runtime_is_missing() {
        let enoent = "OCI runtime exec failed: exec failed: unable to start container process: \
                      exec: \"/usr/local/bin/node\": stat /usr/local/bin/node: no such file or directory: unknown";
        assert!(is_runtime_missing(&report(None, &[enoent])));
        let not_found = "exec: \"npx\": executable file not found in $PATH: unknown";
        assert!(is_runtime_missing(&report(None, &[not_found])));
    }

    #[test]
    fn a_server_that_fails_on_its_own_is_not_a_missing_runtime() {
        assert!(!is_runtime_missing(&report(
            Some(1),
            &["Error: config file /data/a.json: no such file or directory"]
        )));
        assert!(!is_runtime_missing(&report(None, &[])));
    }

    #[test]
    fn message_uses_the_image_of_the_session() {
        let managed = test_docker_session(docker_config(Some("node:22-slim"), None));
        let message = runtime_missing_message("npx", &session_image(&managed));
        assert!(message.contains("'node:22-slim'"));

        let attached = test_docker_session(docker_config(None, Some("dev-box")));
        assert!(session_image(&attached).contains("'dev-box'"));
    }

    #[test]
    fn only_docker_sessions_run_stdio_in_a_container() {
        let docker = test_docker_session(docker_config(Some("python:3.12-slim"), None));
        assert!(StdioExecTarget::from_session(docker.clone())
            .container_session()
            .is_some());

        let mut host = docker;
        host.workspace_isolation = WorkspaceIsolationMode::Host;
        assert!(StdioExecTarget::from_session(host)
            .container_session()
            .is_none());
    }

    #[test]
    fn runtime_missing_message_names_command_and_image() {
        let message = runtime_missing_message("npx", "python:3.12-slim");
        assert!(message.contains("'npx'"));
        assert!(message.contains("'python:3.12-slim'"));
        assert!(message.contains("run in the session container"));
        assert!(message.contains("host path does not resolve"));
    }
}
