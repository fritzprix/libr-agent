use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Represents an interactive browser session, corresponding to a Tauri window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserSession {
    /// A unique identifier for the session.
    pub id: String,
    /// The label used by Tauri to identify the window.
    pub window_label: String,
    /// The current URL of the browser session.
    pub url: String,
    /// The timestamp of when the session was created.
    pub created_at: DateTime<Utc>,
    /// The current status of the session.
    pub status: SessionStatus,
}

/// Represents the status of a `BrowserSession`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionStatus {
    /// The session is in the process of being created.
    Creating,
    /// The session is active and ready for interaction.
    Active,
    /// The session is currently paused.
    Paused,
    /// The session has been closed.
    Closed,
    /// The session has encountered an error.
    Error(String),
}
