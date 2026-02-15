use reqwest::{Client, Method};
use serde_json::Value;
use std::time::Duration;

use crate::repositories::settings_repository::SettingsRepository;
use crate::state::get_settings_repository;
use super::types::SystemSettings;

pub async fn base_url() -> String {
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

pub fn http_client() -> Result<Client, String> {
    Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {e}"))
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
        return Err(format!(
            "Session API {} {} failed ({status}): {}",
            path,
            status.as_u16(),
            text
        ));
    }

    serde_json::from_str(&text).map_err(|e| format!("Invalid JSON response: {e}. body={text}"))
}
