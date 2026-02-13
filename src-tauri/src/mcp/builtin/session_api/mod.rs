use async_trait::async_trait;
use reqwest::{Client, Method};
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Duration;

use crate::mcp::builtin::BuiltinMCPServer;
use crate::mcp::types::{MCPContent, MCPResult, ServiceContext};
use crate::mcp::MCPTool;
use crate::repositories::settings_repository::SettingsRepository;
use crate::state::get_settings_repository;

pub mod tools;

#[derive(Debug, Default)]
pub struct SessionApiServer;

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct SystemSettings {
    http_server_port: Option<u16>,
}

impl SessionApiServer {
    pub fn new() -> Self {
        Self
    }

    pub fn metadata_static() -> crate::mcp::types::BuiltinServerMetadata {
        crate::mcp::types::BuiltinServerMetadata {
            display_name: "Session API".to_string(),
            description: "Client tools for the internal Session Management HTTP API".to_string(),
            icon: None,
        }
    }

    pub fn tools_static() -> Vec<MCPTool> {
        tools::all_tools()
    }

    async fn base_url(&self) -> String {
        let settings_repo = get_settings_repository();

        let port = match settings_repo.get("systemSettings").await {
            Ok(Some(model)) => serde_json::from_str::<SystemSettings>(&model.value)
                .ok()
                .and_then(|s| s.http_server_port)
                .unwrap_or(3030),
            _ => 3030,
        };

        format!("http://127.0.0.1:{}", port)
    }

    fn http_client(&self) -> Result<Client, String> {
        Client::builder()
            .timeout(Duration::from_secs(20))
            .build()
            .map_err(|e| format!("Failed to build HTTP client: {e}"))
    }

    async fn call_json(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
        query: Option<Vec<(String, String)>>,
    ) -> Result<Value, String> {
        let client = self.http_client()?;
        let url = format!("{}{}", self.base_url().await, path);

        let mut req = client.request(method, &url);

        if let Some(q) = query {
            req = req.query(&q);
        }

        if let Some(b) = body {
            req = req.json(&b);
        }

        let resp = req
            .send()
            .await
            .map_err(|e| format!("Session API request failed: {e}"))?;

        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| format!("Failed to read response body: {e}"))?;

        if !status.is_success() {
            return Err(format!(
                "Session API {} {} failed ({status}): {}",
                path,
                status.as_u16(),
                text
            ));
        }

        serde_json::from_str(&text).map_err(|e| format!("Invalid JSON response: {e}. body={text}"))
    }

    fn success_result(text: String, data: Value) -> MCPResult {
        MCPResult {
            content: Some(vec![MCPContent::Text {
                text,
                is_error: None,
            }]),
            structured_content: Some(data),
            is_error: Some(false),
        }
    }

    fn read_required_string(args: &Value, key: &str) -> Result<String, String> {
        args.get(key)
            .and_then(|v| v.as_str())
            .map(|v| v.to_string())
            .ok_or_else(|| format!("Missing required parameter: {key}"))
    }
}

#[async_trait]
impl BuiltinMCPServer for SessionApiServer {
    fn name(&self) -> &str {
        "session_api"
    }

    fn description(&self) -> &str {
        "Client tools for internal HTTP Session Management API"
    }

    fn tools(&self) -> Vec<MCPTool> {
        tools::all_tools()
    }

    async fn call_tool(
        &self,
        tool_name: &str,
        args: Value,
        _session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        match tool_name {
            "healthCheck" => {
                let data = self
                    .call_json(Method::GET, "/api/health", None, None)
                    .await?;
                Ok(Self::success_result(
                    "Session API health check succeeded.".to_string(),
                    data,
                ))
            }
            "createSession" => {
                let assistant_id = Self::read_required_string(&args, "assistantId")?;
                let request = Self::read_required_string(&args, "request")?;

                let mut body = json!({
                    "assistantId": assistant_id,
                    "request": request,
                });

                if let Some(name) = args.get("name").and_then(|v| v.as_str()) {
                    body["name"] = Value::String(name.to_string());
                }

                if let Some(path) = args.get("workspacePath").and_then(|v| v.as_str()) {
                    body["workspacePath"] = Value::String(path.to_string());
                }

                let data = self
                    .call_json(Method::POST, "/api/sessions", Some(body), None)
                    .await?;

                let session_id = data.get("id").and_then(|v| v.as_str()).unwrap_or("unknown");
                let status = data
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");

                Ok(Self::success_result(
                    format!("Session created: {} (status: {})", session_id, status),
                    data,
                ))
            }
            "getSession" => {
                let session_id = Self::read_required_string(&args, "sessionId")?;
                let data = self
                    .call_json(
                        Method::GET,
                        &format!("/api/sessions/{}", session_id),
                        None,
                        None,
                    )
                    .await?;
                Ok(Self::success_result(
                    format!("Fetched session: {}", session_id),
                    data,
                ))
            }
            "getMessages" => {
                let session_id = Self::read_required_string(&args, "sessionId")?;

                let query = args
                    .get("limit")
                    .and_then(|v| v.as_u64())
                    .map(|v| vec![("limit".to_string(), v.to_string())]);

                let data = self
                    .call_json(
                        Method::GET,
                        &format!("/api/sessions/{}/messages", session_id),
                        None,
                        query,
                    )
                    .await?;

                let message_count = data
                    .get("messages")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.len())
                    .unwrap_or(0);

                Ok(Self::success_result(
                    format!(
                        "Fetched {} messages for session {}",
                        message_count, session_id
                    ),
                    data,
                ))
            }
            "sendMessage" => {
                let session_id = Self::read_required_string(&args, "sessionId")?;
                let content = Self::read_required_string(&args, "content")?;

                let data = self
                    .call_json(
                        Method::POST,
                        &format!("/api/sessions/{}/messages", session_id),
                        Some(json!({ "content": content })),
                        None,
                    )
                    .await?;

                let message_id = data.get("id").and_then(|v| v.as_str()).unwrap_or("unknown");
                let status = data
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");

                Ok(Self::success_result(
                    format!("Message accepted: {} (status: {})", message_id, status),
                    data,
                ))
            }
            "terminateSession" => {
                let session_id = Self::read_required_string(&args, "sessionId")?;
                let data = self
                    .call_json(
                        Method::POST,
                        &format!("/api/sessions/{}/terminate", session_id),
                        None,
                        None,
                    )
                    .await?;

                Ok(Self::success_result(
                    format!("Terminated session: {}", session_id),
                    data,
                ))
            }
            "listAssistants" => {
                let data = self
                    .call_json(Method::GET, "/api/assistants", None, None)
                    .await?;

                let assistant_count = data
                    .get("assistants")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.len())
                    .unwrap_or(0);

                Ok(Self::success_result(
                    format!("Fetched {} assistants", assistant_count),
                    data,
                ))
            }
            _ => Err(format!("Unknown tool: {}", tool_name)),
        }
    }

    async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
        let base_url = self.base_url().await;

        ServiceContext {
            context_prompt: format!(
                "## Session API\n\nInternal API client is available at {}\nUse these tools to create/manage nested sessions.",
                base_url
            ),
            structured_state: Some(json!({
                "base_url": base_url,
                "server": "session_api"
            })),
        }
    }
}
