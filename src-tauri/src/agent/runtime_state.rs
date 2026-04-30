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
pub struct SessionRuntimeInitializationState {
    pub current_step: Option<String>,
    pub result: SessionRuntimeInitResult,
    pub error: Option<String>,
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
    pub phase: SessionRuntimePhase,
    pub proxy: SessionRuntimeProxyState,
    pub initialization: SessionRuntimeInitializationState,
    #[serde(default)]
    pub servers: Vec<SessionRuntimeServerState>,
}

impl Default for SessionRuntimeState {
    fn default() -> Self {
        Self {
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
                }
            },
            ..Self::default()
        }
    }

    pub fn builtin_ready() -> Self {
        Self {
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
            },
            servers: Vec::new(),
        }
    }

    pub fn configured_initializing(servers: Vec<SessionRuntimeServerState>) -> Self {
        Self {
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

    pub fn recompute_summary(&mut self) {
        let total_servers = self.servers.len();
        let ready_servers = self
            .servers
            .iter()
            .filter(|server| server.status == SessionRuntimeServerStatus::Ready)
            .count();
        let failed_servers = self
            .servers
            .iter()
            .filter(|server| server.status == SessionRuntimeServerStatus::Failed)
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

        if failed_servers > 0 && ready_servers > 0 {
            self.phase = SessionRuntimePhase::Degraded;
            self.proxy.ready = true;
            self.initialization.result = SessionRuntimeInitResult::Partial;
            self.initialization.error = Some(format!(
                "{} of {} external servers failed during initialization",
                failed_servers, total_servers
            ));
            return;
        }

        if failed_servers == total_servers {
            self.phase = SessionRuntimePhase::Failed;
            self.proxy.ready = false;
            self.initialization.result = SessionRuntimeInitResult::Failed;
            self.initialization.error = self
                .servers
                .iter()
                .find_map(|server| server.error.clone())
                .or_else(|| Some("All external servers failed during initialization".to_string()));
            return;
        }

        self.phase = SessionRuntimePhase::Initializing;
        self.proxy.ready = false;
        self.initialization.result = SessionRuntimeInitResult::Pending;
    }
}
