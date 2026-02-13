use serde::Deserialize;

#[derive(Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SystemSettings {
    pub search_index_frequency_minutes: Option<u64>,
    pub web_action_timeout_seconds: Option<u64>,
    pub active_session_retention_hours: Option<u64>,
    pub http_server_port: Option<u16>,
    pub http_server_expose: Option<bool>,
}
