use reqwest::{Client, Method};
use serde_json::Value;
use std::time::Duration;
use tokio::time::sleep;

use crate::state::get_settings_repository;
use crate::repositories::settings_repository::SettingsRepository;
use super::types::SystemSettings;
use super::formatting::{extract_session_status, is_terminal_status};

#[derive(Debug, Default, Clone)]
pub struct SessionApiClient;

impl SessionApiClient {
    pub fn new() -> Self {
        Self
    }

    pub async fn base_url(&self) -> String {
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

    pub fn http_client(&self) -> Result<Client, String> {
        Client::builder()
            .timeout(Duration::from_secs(20))
            .build()
            .map_err(|e| format!("Failed to build HTTP client: {e}"))
    }

    pub async fn call_json(
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

    pub async fn wait_until_session_terminal(
        &self,
        session_id: &str,
        timeout_seconds: u64,
        poll_interval_seconds: u64,
    ) -> Result<(Value, u64), String> {
        let timeout_seconds = timeout_seconds.clamp(5, 900);
        let poll_interval_seconds = poll_interval_seconds.clamp(1, 30);

        let started_at = std::time::Instant::now();
        let mut poll_count: u64 = 0;

        loop {
            let session = self
                .call_json(
                    Method::GET,
                    &format!("/api/sessions/{}", session_id),
                    None,
                    None,
                )
                .await?;

            poll_count = poll_count.saturating_add(1);
            let status = extract_session_status(&session);
            if is_terminal_status(&status) {
                return Ok((session, poll_count));
            }

            if started_at.elapsed() >= Duration::from_secs(timeout_seconds) {
                return Err(format!(
                    "waitForSessionIdle timed out after {}s for session {}",
                    timeout_seconds, session_id
                ));
            }

            sleep(Duration::from_secs(poll_interval_seconds)).await;
        }
    }
}
