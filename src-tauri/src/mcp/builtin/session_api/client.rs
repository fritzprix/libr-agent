use reqwest::{Client, Method};
use serde_json::Value;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::sync::OnceCell;

use super::types::SystemSettings;
use crate::repositories::settings_repository::SettingsRepository;
use crate::state::get_settings_repository;

static HTTP_CLIENT: OnceLock<Client> = OnceLock::new();
static BASE_URL: OnceCell<String> = OnceCell::const_new();

pub fn http_client() -> Result<&'static Client, String> {
    Ok(HTTP_CLIENT.get_or_init(|| {
        Client::builder()
            // Extended timeout to accommodate child session creation latency:
            // external MCP server startup (stdio spawn + tool discovery) can take 10-30s.
            .timeout(Duration::from_secs(120))
            .build()
            .expect("Failed to build HTTP client")
    }))
}

pub async fn base_url() -> &'static str {
    BASE_URL
        .get_or_init(|| async {
            let settings_repo = get_settings_repository();
            let port = match settings_repo.get("systemSettings").await {
                Ok(Some(model)) => serde_json::from_str::<SystemSettings>(&model.value)
                    .ok()
                    .and_then(|s| s.http_server_port)
                    .unwrap_or(3030),
                _ => 3030,
            };
            format!("http://127.0.0.1:{}", port)
        })
        .await
}

pub async fn call_json(
    method: Method,
    path: &str,
    body: Option<Value>,
    query: Option<Vec<(String, String)>>,
) -> Result<Value, String> {
    let client = http_client()?;
    let url = format!("{}{}", base_url().await, path);

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
        // Include status code (needed by error_normalization::extract_http_status for
        // correct error categorization) but omit the internal API path.
        let cause = serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .and_then(|v| v["error"].as_str().map(String::from))
            .unwrap_or_else(|| {
                status
                    .canonical_reason()
                    .unwrap_or("Unknown error")
                    .to_string()
            });
        return Err(format!("Request failed ({}): {}", status.as_u16(), cause));
    }

    serde_json::from_str(&text).map_err(|e| format!("Invalid JSON response: {e}. body={text}"))
}
