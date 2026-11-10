use serde::Deserialize;

/// Arguments for the `listSessions` handler.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListArgs {
    pub agent_id: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub status: Option<String>,
    pub page: Option<u64>,
    pub page_size: Option<u64>,
}

/// Arguments for the `readSession` handler.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadSessionArgs {
    pub session_id: String,
    pub page: Option<u64>,
    pub page_size: Option<u64>,
    /// `"asc"` (oldest first) or `"desc"` (newest first). Defaults to `"desc"`.
    pub order: Option<String>,
}

/// Arguments for the `readMessage` handler.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadMessageArgs {
    pub message_id: String,
    pub offset_chars: Option<usize>,
    pub max_chars: Option<usize>,
}

/// Arguments for the `searchHistory` handler.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchArgs {
    pub query: String,
    pub agent_id: Option<String>,
    pub session_id: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub roles: Option<Vec<String>>,
    pub page: Option<u64>,
    pub page_size: Option<u64>,
}
