use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionRuntimePhase {
    NotStarted,
    Hydrating,
    Initializing,
    Ready,
    Degraded,
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionRuntimeInitResult {
    Pending,
    Success,
    Partial,
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionRuntimeProxyMode {
    None,
    BuiltinOnly,
    Configured,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionRuntimeTransport {
    Stdio,
    Http,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionRuntimeServerStatus {
    NotStarted,
    Connecting,
    DiscoveringTools,
    Ready,
    Failed,
    /// Discovery deadline or soft wait timed out before this server finished.
    TimedOut,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionRuntimeProxyState {
    pub exists: bool,
    pub mode: SessionRuntimeProxyMode,
    pub ready: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionRuntimeDockerState {
    /// Managed image ref, or `attach:<container>` for attach sessions. Absent when unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    pub step: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub progress: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionRuntimeInitializationState {
    pub current_step: Option<String>,
    pub result: SessionRuntimeInitResult,
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub docker: Option<SessionRuntimeDockerState>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionRuntimeServerState {
    pub name: String,
    pub transport: SessionRuntimeTransport,
    pub status: SessionRuntimeServerStatus,
    pub tool_count: usize,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionRuntimeState {
    #[serde(default)]
    pub sequence: u64,
    pub phase: SessionRuntimePhase,
    pub proxy: SessionRuntimeProxyState,
    pub initialization: SessionRuntimeInitializationState,
    #[serde(default)]
    pub servers: Vec<SessionRuntimeServerState>,
}

impl Default for SessionRuntimeState {
    fn default() -> Self {
        Self {
            sequence: 0,
            phase: SessionRuntimePhase::NotStarted,
            proxy: SessionRuntimeProxyState {
                exists: false,
                mode: SessionRuntimeProxyMode::None,
                ready: false,
            },
            initialization: SessionRuntimeInitializationState {
                current_step: None,
                result: SessionRuntimeInitResult::Pending,
                error: None,
                docker: None,
            },
            servers: Vec::new(),
        }
    }
}

impl SessionRuntimeState {
    pub fn hydrating() -> Self {
        Self {
            phase: SessionRuntimePhase::Hydrating,
            initialization: SessionRuntimeInitializationState {
                current_step: Some("Starting session...".to_string()),
                ..SessionRuntimeInitializationState {
                    current_step: None,
                    result: SessionRuntimeInitResult::Pending,
                    error: None,
                    docker: None,
                }
            },
            ..Self::default()
        }
    }

    pub fn builtin_ready() -> Self {
        Self {
            sequence: 0,
            phase: SessionRuntimePhase::Ready,
            proxy: SessionRuntimeProxyState {
                exists: true,
                mode: SessionRuntimeProxyMode::BuiltinOnly,
                ready: true,
            },
            initialization: SessionRuntimeInitializationState {
                current_step: Some("Session initialization complete".to_string()),
                result: SessionRuntimeInitResult::Success,
                error: None,
                docker: None,
            },
            servers: Vec::new(),
        }
    }

    pub fn configured_initializing(servers: Vec<SessionRuntimeServerState>) -> Self {
        Self {
            sequence: 0,
            phase: SessionRuntimePhase::Initializing,
            proxy: SessionRuntimeProxyState {
                exists: false,
                mode: SessionRuntimeProxyMode::Configured,
                ready: false,
            },
            initialization: SessionRuntimeInitializationState {
                current_step: Some("Initializing session services".to_string()),
                result: SessionRuntimeInitResult::Pending,
                error: None,
                docker: None,
            },
            servers,
        }
    }

    pub fn set_proxy_exists(&mut self, exists: bool) {
        self.proxy.exists = exists;
    }

    pub fn set_current_step(&mut self, step: impl Into<String>) {
        self.initialization.current_step = Some(step.into());
    }

    pub fn upsert_server(
        &mut self,
        name: &str,
        transport: SessionRuntimeTransport,
        status: SessionRuntimeServerStatus,
        tool_count: usize,
        error: Option<String>,
    ) {
        if let Some(server) = self.servers.iter_mut().find(|server| server.name == name) {
            server.transport = transport;
            server.status = status;
            server.tool_count = tool_count;
            server.error = error;
        } else {
            self.servers.push(SessionRuntimeServerState {
                name: name.to_string(),
                transport,
                status,
                tool_count,
                error,
            });
        }
    }

    pub fn is_terminal_server_status(status: &SessionRuntimeServerStatus) -> bool {
        matches!(
            status,
            SessionRuntimeServerStatus::Ready
                | SessionRuntimeServerStatus::Failed
                | SessionRuntimeServerStatus::TimedOut
        )
    }

    pub fn is_unsuccessful_terminal_status(status: &SessionRuntimeServerStatus) -> bool {
        matches!(
            status,
            SessionRuntimeServerStatus::Failed | SessionRuntimeServerStatus::TimedOut
        )
    }

    /// Mark still-pending servers as timed out and recompute Session Ready.
    /// Idempotent when initialization has already left `pending`.
    pub fn finalize_discovery_timeout(&mut self, reason: impl Into<String>) -> bool {
        if self.initialization.result != SessionRuntimeInitResult::Pending {
            return false;
        }

        let reason = reason.into();
        for server in &mut self.servers {
            if !Self::is_terminal_server_status(&server.status) {
                server.status = SessionRuntimeServerStatus::TimedOut;
                server.error = Some(reason.clone());
            }
        }

        self.recompute_summary();
        true
    }

    pub fn recompute_summary(&mut self) {
        let total_servers = self.servers.len();
        let ready_servers = self
            .servers
            .iter()
            .filter(|server| server.status == SessionRuntimeServerStatus::Ready)
            .count();
        let unsuccessful_servers = self
            .servers
            .iter()
            .filter(|server| Self::is_unsuccessful_terminal_status(&server.status))
            .count();

        if self.proxy.mode == SessionRuntimeProxyMode::None {
            self.phase = SessionRuntimePhase::NotStarted;
            self.proxy.ready = false;
            self.initialization.result = SessionRuntimeInitResult::Pending;
            return;
        }

        if total_servers == 0 {
            self.phase = SessionRuntimePhase::Ready;
            self.proxy.ready = self.proxy.exists;
            self.initialization.result = SessionRuntimeInitResult::Success;
            self.initialization.error = None;
            return;
        }

        if ready_servers == total_servers {
            self.phase = SessionRuntimePhase::Ready;
            self.proxy.ready = true;
            self.initialization.result = SessionRuntimeInitResult::Success;
            self.initialization.error = None;
            return;
        }

        if unsuccessful_servers > 0 && ready_servers > 0 {
            self.phase = SessionRuntimePhase::Degraded;
            self.proxy.ready = true;
            self.initialization.result = SessionRuntimeInitResult::Partial;
            self.initialization.error = Some(format!(
                "{} of {} external servers failed or timed out during initialization",
                unsuccessful_servers, total_servers
            ));
            return;
        }

        if unsuccessful_servers == total_servers {
            self.phase = SessionRuntimePhase::Failed;
            // Builtin tools remain usable when the proxy exists; external MCP
            // failure must not permanently block chat/send.
            self.proxy.ready = self.proxy.exists;
            self.initialization.result = SessionRuntimeInitResult::Failed;
            self.initialization.error = self
                .servers
                .iter()
                .find_map(|server| server.error.clone())
                .or_else(|| {
                    Some("All external servers failed or timed out during initialization".to_string())
                });
            return;
        }

        self.phase = SessionRuntimePhase::Initializing;
        self.proxy.ready = false;
        self.initialization.result = SessionRuntimeInitResult::Pending;
    }
}
