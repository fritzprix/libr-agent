use super::WorkspaceServer;
use crate::{mcp::MCPResponse, session_isolation::IsolationLevel};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Deserialize)]
struct OpenTerminalArgs {
    command: String,
    args: Option<Vec<String>>,
    working_dir: Option<String>,
    env: Option<HashMap<String, String>>,
    isolation: Option<String>,
}

#[derive(Deserialize)]
struct CloseTerminalArgs {
    terminal_id: String,
}

#[derive(Deserialize)]
struct ReadTerminalArgs {
    terminal_id: String,
    from_seq: Option<u64>,
}

#[derive(Deserialize)]
struct ListTerminalsArgs {}

impl WorkspaceServer {
    pub async fn handle_open_terminal(&self, args: Value) -> MCPResponse {
        let request_id = Self::generate_request_id();
        let parsed_args: OpenTerminalArgs = match serde_json::from_value(args) {
            Ok(a) => a,
            Err(e) => return Self::error_response(request_id, -32602, &e.to_string()),
        };

        let session_id = self
            .session_manager
            .get_current_session()
            .unwrap_or_else(|| "default".to_string());
        let workspace_dir = self.get_workspace_dir();
        let working_dir = parsed_args
            .working_dir
            .map(PathBuf::from)
            .or(Some(workspace_dir));

        let isolation_level = parsed_args
            .isolation
            .map(|level| match level.as_str() {
                "basic" => IsolationLevel::Basic,
                "high" => IsolationLevel::High,
                _ => IsolationLevel::Medium,
            })
            .unwrap_or(IsolationLevel::Medium);

        match self
            .terminal_manager
            .open_terminal(
                parsed_args.command,
                parsed_args.args.unwrap_or_default(),
                working_dir,
                parsed_args.env.unwrap_or_default(),
                isolation_level,
                session_id,
                &self.isolation_manager,
            )
            .await
        {
            Ok(terminal_id) => Self::success_response(request_id, &terminal_id),
            Err(e) => Self::error_response(request_id, -32603, &e),
        }
    }

    pub async fn handle_close_terminal(&self, args: Value) -> MCPResponse {
        let request_id = Self::generate_request_id();
        let parsed_args: CloseTerminalArgs = match serde_json::from_value(args) {
            Ok(a) => a,
            Err(e) => return Self::error_response(request_id, -32602, &e.to_string()),
        };

        match self
            .terminal_manager
            .close_terminal(&parsed_args.terminal_id)
            .await
        {
            Ok(result) => {
                let result_json = serde_json::to_string(&result).unwrap_or_default();
                Self::success_response(request_id, &result_json)
            }
            Err(e) => Self::error_response(request_id, -32603, &e),
        }
    }

    pub async fn handle_read_terminal(&self, args: Value) -> MCPResponse {
        let request_id = Self::generate_request_id();
        let parsed_args: ReadTerminalArgs = match serde_json::from_value(args) {
            Ok(a) => a,
            Err(e) => return Self::error_response(request_id, -32602, &e.to_string()),
        };

        match self
            .terminal_manager
            .read_terminal(&parsed_args.terminal_id, parsed_args.from_seq.unwrap_or(0))
            .await
        {
            Ok(output) => {
                let output_json = serde_json::to_string(&output).unwrap_or_default();
                Self::success_response(request_id, &output_json)
            }
            Err(e) => Self::error_response(request_id, -32603, &e),
        }
    }

    pub async fn handle_list_terminals(&self, _args: Value) -> MCPResponse {
        let request_id = Self::generate_request_id();
        let session_id = self
            .session_manager
            .get_current_session()
            .unwrap_or_else(|| "default".to_string());

        let summaries = self
            .terminal_manager
            .list_terminals(Some(&session_id))
            .await;
        let summaries_json = serde_json::to_string(&summaries).unwrap_or_default();
        Self::success_response(request_id, &summaries_json)
    }
}
