use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PageClassification {
    Normal,
    BlockedInterstitial,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HistoryNavigationStatus {
    Navigated,
    NoHistoryEntry,
    BlockedInterstitial,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageState {
    pub url: String,
    pub title: Option<String>,
    pub classification: Option<PageClassification>,
    pub navigation_status: Option<HistoryNavigationStatus>,
    pub navigation_message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct SidecarRequest {
    pub(crate) id: String,
    pub(crate) method: String,
    pub(crate) params: Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct SidecarResponse {
    pub(crate) id: String,
    pub(crate) result: Option<Value>,
    pub(crate) error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CreateSessionParams {
    pub(crate) session_id: String,
    pub(crate) url: String,
    pub(crate) title: Option<String>,
    pub(crate) visible: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SessionIdParams {
    pub(crate) session_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NavigateParams {
    pub(crate) session_id: String,
    pub(crate) url: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EvaluateParams {
    pub(crate) session_id: String,
    pub(crate) script: String,
}
