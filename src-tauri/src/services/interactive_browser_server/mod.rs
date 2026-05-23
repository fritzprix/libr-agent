use chrono::Utc;
use log::{debug, error, info, warn};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use crate::browser_sidecar::{BrowserAutomationClient, HistoryNavigationStatus};

use super::browser_error::BrowserError;

pub mod id_gen;
pub use id_gen::generate_session_id;

pub mod types;
pub use types::{BrowserSession, NavigationUpdateOutcome, SessionStatus};

pub mod utils;
pub use utils::validate_and_normalize_url;

#[derive(Clone)]
pub struct InteractiveBrowserServer {
    sessions: Arc<RwLock<HashMap<String, BrowserSession>>>,
    client: BrowserAutomationClient,
}

impl InteractiveBrowserServer {
    pub fn new(action_timeout: Duration) -> Self {
        info!(
            "Initializing Interactive Browser Server with browser sidecar backend and timeout: {:?}",
            action_timeout
        );

        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            client: BrowserAutomationClient::new(action_timeout),
        }
    }

    pub fn action_timeout(&self) -> Duration {
        self.client.request_timeout()
    }

    pub async fn create_browser_session(
        &self,
        url: &str,
        title: Option<&str>,
        visible: bool,
    ) -> Result<(String, String), String> {
        if url.trim().is_empty() {
            return Err("The 'url' parameter is required".to_string());
        }

        let validated_url = validate_and_normalize_url(url)?;
        let session_id = generate_session_id();
        let session = BrowserSession {
            id: session_id.clone(),
            url: validated_url.clone(),
            current_title: None,
            created_at: Utc::now(),
            status: SessionStatus::Creating,
            page_generation: 1,
            runtime_ready_generation: None,
        };

        {
            let mut sessions = self
                .sessions
                .write()
                .map_err(|e| format!("Failed to acquire write lock: {e}"))?;
            sessions.insert(session_id.clone(), session);
        }

        info!("Creating browser sidecar session {session_id} for URL: {validated_url}");
        let create_result = self
            .client
            .create_session(&session_id, &validated_url, title, visible)
            .await;

        match create_result {
            Ok(state) => {
                let mut sessions = self
                    .sessions
                    .write()
                    .map_err(|e| format!("Failed to acquire write lock: {e}"))?;
                if let Some(session) = sessions.get_mut(&session_id) {
                    session.url = state.url.clone();
                    session.current_title = state.title.clone();
                    session.status = SessionStatus::Active;
                    session.runtime_ready_generation = Some(session.page_generation);
                }

                let message = format!(
                    "Session created for {} - active session ready for content extraction",
                    state.url
                );
                Ok((session_id, message))
            }
            Err(error) => {
                if let Ok(mut sessions) = self.sessions.write() {
                    sessions.remove(&session_id);
                }
                Err(format!(
                    "Failed to create browser automation session: {error}"
                ))
            }
        }
    }

    pub async fn execute_script(&self, session_id: &str, script: &str) -> Result<String, String> {
        debug!("Executing browser script in session {session_id}: {script}");
        let session = self.get_session(session_id)?;
        match &session.status {
            SessionStatus::Active => {}
            SessionStatus::Error(message) => {
                return Err(format!(
                    "Browser session {} is in an error state: {}. Close the session or create a new one.",
                    session_id, message
                ));
            }
            _ => {
                return Err(format!(
                    "Browser session {} is not ready for script execution",
                    session_id
                ));
            }
        }

        self.client.evaluate(session_id, script).await
    }

    pub fn list_sessions(&self) -> Vec<BrowserSession> {
        match self.sessions.read() {
            Ok(sessions) => sessions
                .values()
                .filter(|session| !matches!(session.status, SessionStatus::Closed))
                .cloned()
                .collect(),
            Err(error) => {
                error!("Failed to list browser sessions: {error}");
                Vec::new()
            }
        }
    }

    pub fn get_session(&self, session_id: &str) -> Result<BrowserSession, String> {
        let sessions = self.sessions.read().map_err(|e| {
            String::from(BrowserError::LockFailed {
                reason: format!("Failed to acquire read lock: {e}"),
            })
        })?;

        sessions.get(session_id).cloned().ok_or_else(|| {
            String::from(BrowserError::SessionNotFound {
                session_id: session_id.to_string(),
            })
        })
    }

    pub async fn close_session(&self, session_id: &str) -> Result<String, String> {
        info!("Closing browser session: {session_id}");
        self.get_session(session_id)?;

        let close_result = self.client.close_session(session_id).await;
        let recovered = match close_result {
            Ok(()) => false,
            Err(error) if is_recoverable_sidecar_failure(&error) => {
                warn!(
                    "Recovering browser session {} after sidecar failure during close: {}",
                    session_id, error
                );
                true
            }
            Err(error) => return Err(error),
        };

        {
            let mut sessions = self
                .sessions
                .write()
                .map_err(|e| format!("Failed to acquire write lock: {e}"))?;
            sessions.remove(session_id);
        }

        let message = if recovered {
            "Session closed after recovering from a browser sidecar failure".to_string()
        } else {
            "Session closed successfully".to_string()
        };
        info!(
            "Browser automation session closed successfully: {}",
            session_id
        );
        Ok(message)
    }

    pub async fn close_all_sessions(&self) -> Result<(), String> {
        let session_ids: Vec<String> = {
            let sessions = self
                .sessions
                .read()
                .map_err(|e| format!("Failed to acquire read lock for listing sessions: {e}"))?;
            sessions.keys().cloned().collect()
        };

        for session_id in session_ids {
            if let Err(error) = self.close_session(&session_id).await {
                warn!(
                    "Failed to close browser session {} during shutdown: {}",
                    session_id, error
                );
            }
        }

        Ok(())
    }

    pub async fn navigate_to_url(&self, session_id: &str, url: &str) -> Result<String, String> {
        let current_session = self.get_session(session_id)?;
        let target_url = resolve_target_url(url, &current_session.url)?;
        let next_generation = self.begin_navigation(session_id, Some(target_url.clone()))?;
        let state = match self.client.navigate(session_id, &target_url).await {
            Ok(state) => state,
            Err(error) => {
                self.mark_navigation_error(
                    session_id,
                    next_generation,
                    format!("Navigation failed: {error}"),
                )?;
                return Err(format!("Navigation failed: {error}"));
            }
        };

        let outcome =
            self.finish_navigation(session_id, next_generation, &state.url, state.title.clone())?;
        self.require_applied_navigation("Navigation", session_id, next_generation, outcome)?;

        let message = format!(
            "Navigated active session to {} - ready for content extraction",
            state.url
        );
        Ok(message)
    }

    pub async fn navigate_back(&self, session_id: &str) -> Result<String, String> {
        let next_generation = self.begin_navigation(session_id, None)?;
        let state = match self.client.go_back(session_id).await {
            Ok(state) => state,
            Err(error) => {
                self.mark_navigation_error(
                    session_id,
                    next_generation,
                    format!("Back navigation failed: {error}"),
                )?;
                return Err(format!("Back navigation failed: {error}"));
            }
        };
        let outcome =
            self.finish_navigation(session_id, next_generation, &state.url, state.title.clone())?;
        self.require_applied_navigation("Back navigation", session_id, next_generation, outcome)?;
        Ok(Self::describe_history_navigation("back", state))
    }

    pub async fn navigate_forward(&self, session_id: &str) -> Result<String, String> {
        let next_generation = self.begin_navigation(session_id, None)?;
        let state = match self.client.go_forward(session_id).await {
            Ok(state) => state,
            Err(error) => {
                self.mark_navigation_error(
                    session_id,
                    next_generation,
                    format!("Forward navigation failed: {error}"),
                )?;
                return Err(format!("Forward navigation failed: {error}"));
            }
        };
        let outcome =
            self.finish_navigation(session_id, next_generation, &state.url, state.title.clone())?;
        self.require_applied_navigation(
            "Forward navigation",
            session_id,
            next_generation,
            outcome,
        )?;
        Ok(Self::describe_history_navigation("forward", state))
    }

    fn describe_history_navigation(
        direction: &str,
        state: crate::browser_sidecar::PageState,
    ) -> String {
        match state.navigation_status {
            Some(HistoryNavigationStatus::Navigated) | None => format!("Navigated {}", direction),
            Some(HistoryNavigationStatus::NoHistoryEntry) => {
                state.navigation_message.unwrap_or_else(|| {
                    format!(
                        "No {} history entry produced an observable page change; staying on {}",
                        direction, state.url
                    )
                })
            }
            Some(HistoryNavigationStatus::BlockedInterstitial) => {
                state.navigation_message.unwrap_or_else(|| {
                    format!(
                        "{} navigation is blocked by an interstitial or CAPTCHA page at {}",
                        direction, state.url
                    )
                })
            }
        }
    }

    fn begin_navigation(&self, session_id: &str, next_url: Option<String>) -> Result<u64, String> {
        let mut sessions = self.sessions.write().map_err(|e| {
            String::from(BrowserError::LockFailed {
                reason: format!("Failed to acquire write lock: {e}"),
            })
        })?;
        let session = sessions.get_mut(session_id).ok_or_else(|| {
            String::from(BrowserError::SessionNotFound {
                session_id: session_id.to_string(),
            })
        })?;
        Ok(session.begin_navigation(next_url))
    }

    fn finish_navigation(
        &self,
        session_id: &str,
        generation: u64,
        url: &str,
        title: Option<String>,
    ) -> Result<NavigationUpdateOutcome, String> {
        let mut sessions = self
            .sessions
            .write()
            .map_err(|e| format!("Failed to acquire write lock: {e}"))?;
        let session = sessions.get_mut(session_id).ok_or_else(|| {
            String::from(BrowserError::SessionNotFound {
                session_id: session_id.to_string(),
            })
        })?;
        let outcome = session.finish_navigation(generation, url, title);
        match outcome {
            NavigationUpdateOutcome::Applied => {}
            NavigationUpdateOutcome::IgnoredStale => {
                warn!(
                    "Ignoring stale navigation completion for session {} generation {} (current {})",
                    session_id, generation, session.page_generation
                );
            }
            NavigationUpdateOutcome::IgnoredSettled => {
                debug!(
                    "Ignoring duplicate navigation completion for session {} generation {} with status {:?}",
                    session_id, generation, session.status
                );
            }
            NavigationUpdateOutcome::RejectedFuture => {
                warn!(
                    "Ignoring future navigation completion for session {} generation {} (current {})",
                    session_id, generation, session.page_generation
                );
            }
        }
        Ok(outcome)
    }

    fn mark_navigation_error(
        &self,
        session_id: &str,
        generation: u64,
        error_message: String,
    ) -> Result<(), String> {
        let mut sessions = self
            .sessions
            .write()
            .map_err(|e| format!("Failed to acquire write lock: {e}"))?;
        if let Some(session) = sessions.get_mut(session_id) {
            match session.fail_navigation(generation, error_message) {
                NavigationUpdateOutcome::Applied => {}
                NavigationUpdateOutcome::IgnoredStale => {
                    warn!(
                        "Ignoring stale navigation error for session {} generation {} (current {})",
                        session_id, generation, session.page_generation
                    );
                }
                NavigationUpdateOutcome::IgnoredSettled => {
                    debug!(
                        "Ignoring duplicate navigation error for session {} generation {} with status {:?}",
                        session_id, generation, session.status
                    );
                }
                NavigationUpdateOutcome::RejectedFuture => {
                    warn!(
                        "Ignoring future navigation error for session {} generation {} (current {})",
                        session_id, generation, session.page_generation
                    );
                }
            }
        }
        Ok(())
    }

    fn require_applied_navigation(
        &self,
        operation: &str,
        session_id: &str,
        generation: u64,
        outcome: NavigationUpdateOutcome,
    ) -> Result<(), String> {
        match outcome {
            NavigationUpdateOutcome::Applied => Ok(()),
            NavigationUpdateOutcome::IgnoredStale => Err(format!(
                "{} result for session {} was superseded by a newer navigation request (generation {})",
                operation, session_id, generation
            )),
            NavigationUpdateOutcome::IgnoredSettled => Err(format!(
                "{} result for session {} was ignored because the session had already settled generation {}",
                operation, session_id, generation
            )),
            NavigationUpdateOutcome::RejectedFuture => Err(format!(
                "{} result for session {} referenced an invalid future generation {}",
                operation, session_id, generation
            )),
        }
    }
}

fn is_recoverable_sidecar_failure(error: &str) -> bool {
    let lower = error.to_ascii_lowercase();
    lower.contains("browser sidecar was reset")
        || lower.contains("browser sidecar is not running")
        || lower.contains("browser sidecar did not respond within")
        || lower.contains("browser sidecar response channel closed unexpectedly")
        || lower.contains("browser sidecar exited unexpectedly")
        || lower.contains("browser sidecar closed its stdout")
        || lower.contains("failed reading browser sidecar stdout")
        || lower.contains("browser session not found")
}

fn resolve_target_url(url: &str, current_url: &str) -> Result<String, String> {
    match url::Url::parse(url) {
        Ok(parsed) => match parsed.scheme() {
            "http" | "https" | "about" => Ok(url.to_string()),
            scheme => Err(format!(
                "Unsupported URL scheme '{}'. Allowed: http://, https://, about:",
                scheme
            )),
        },
        Err(_) => {
            let with_proto = format!("https://{}", url);
            if url::Url::parse(&with_proto).is_ok() {
                return Ok(with_proto);
            }
            let base = url::Url::parse(current_url)
                .map_err(|e| format!("Current session URL is invalid: {e}"))?;
            let joined = base
                .join(url)
                .map_err(|e| format!("Failed to resolve relative URL: {e}"))?;
            warn!("Detected relative URL '{}'. Resolved to '{}'", url, joined);
            Ok(joined.to_string())
        }
    }
}
