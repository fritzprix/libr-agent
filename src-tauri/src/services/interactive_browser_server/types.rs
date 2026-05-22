use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Represents an interactive browser session, corresponding to a Tauri window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserSession {
    /// A unique identifier for the session.
    pub id: String,
    /// High-entropy token required for webview-to-Rust IPC messages for this session.
    #[serde(skip_serializing, skip_deserializing)]
    pub ipc_token: String,
    /// The label used by Tauri to identify the window.
    pub window_label: String,
    /// The current URL of the browser session.
    pub url: String,
    /// The last known page title reported by the browser runtime.
    pub current_title: Option<String>,
    /// The timestamp of when the session was created.
    pub created_at: DateTime<Utc>,
    /// The current status of the session.
    pub status: SessionStatus,
    /// Monotonic page generation used to distinguish navigations.
    pub page_generation: u64,
    /// The generation that most recently reported a ready browser runtime.
    pub runtime_ready_generation: Option<u64>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationUpdateOutcome {
    Applied,
    IgnoredStale,
    IgnoredSettled,
    RejectedFuture,
}

impl BrowserSession {
    pub fn begin_navigation(&mut self, next_url: Option<String>) -> u64 {
        self.page_generation += 1;
        self.runtime_ready_generation = None;
        self.status = SessionStatus::Creating;
        self.current_title = None;
        if let Some(url) = next_url {
            self.url = url;
        }
        self.page_generation
    }

    pub fn finish_navigation(
        &mut self,
        generation: u64,
        url: &str,
        title: Option<String>,
    ) -> NavigationUpdateOutcome {
        use std::cmp::Ordering;

        match generation.cmp(&self.page_generation) {
            Ordering::Less => NavigationUpdateOutcome::IgnoredStale,
            Ordering::Greater => NavigationUpdateOutcome::RejectedFuture,
            Ordering::Equal => {
                if !matches!(self.status, SessionStatus::Creating) {
                    return NavigationUpdateOutcome::IgnoredSettled;
                }

                self.runtime_ready_generation = Some(generation);
                self.status = SessionStatus::Active;
                self.url = url.to_string();
                self.current_title = title;
                NavigationUpdateOutcome::Applied
            }
        }
    }

    pub fn fail_navigation(
        &mut self,
        generation: u64,
        error_message: String,
    ) -> NavigationUpdateOutcome {
        use std::cmp::Ordering;

        match generation.cmp(&self.page_generation) {
            Ordering::Less => NavigationUpdateOutcome::IgnoredStale,
            Ordering::Greater => NavigationUpdateOutcome::RejectedFuture,
            Ordering::Equal => {
                if !matches!(self.status, SessionStatus::Creating) {
                    return NavigationUpdateOutcome::IgnoredSettled;
                }

                self.runtime_ready_generation = None;
                self.status = SessionStatus::Error(error_message);
                NavigationUpdateOutcome::Applied
            }
        }
    }

    pub fn is_runtime_ready(&self) -> bool {
        self.runtime_ready_generation == Some(self.page_generation)
    }
}
