use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use tokio::sync::oneshot;

/// Platform-specific persistent shell tool name
#[cfg(unix)]
pub const PERSISTENT_SHELL_TOOL: &str = "runInPersistentShell";
#[cfg(windows)]
pub const PERSISTENT_SHELL_TOOL: &str = "runInPersistentPowerShell";

/// Platform-specific one-shot shell tool name
#[cfg(unix)]
pub const RUN_SHELL_TOOL: &str = "runShell";
#[cfg(windows)]
pub const RUN_SHELL_TOOL: &str = "runPowerShell";

pub(crate) const SUBMIT_INTERACTIVE_SHELL_INPUT_INTERNAL: &str = "submitInteractiveShellInput";
pub(crate) const CANCEL_INTERACTIVE_SHELL_INPUT_INTERNAL: &str = "cancelInteractiveShellInput";
pub(crate) const INTERACTIVE_SHELL_INPUT_TIMEOUT_SECS: u64 = 300;
pub(crate) const INTERACTIVE_SHELL_INPUT_MAX_BYTES: usize = 65_536;

/// Pending execution state (server-side only)
/// Stores metadata for shell commands awaiting user input
pub enum PendingShellInputResolution {
    Submitted(String),
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingExecutionLookupError {
    SessionMismatch,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum InteractiveShellInputType {
    Text,
    Password,
}

impl InteractiveShellInputType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Password => "password",
        }
    }
}

impl std::fmt::Display for InteractiveShellInputType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

pub struct PendingShellExecution {
    pub execution_id: String,
    pub session_id: String,
    pub executable_command: String, // Command to execute (may include -S flag)
    pub display_command: String,    // Sanitized version for logs/UI
    pub run_mode: String,           // "sync" or "async" from 1st call
    pub timeout: u64,               // Command execution timeout in seconds
    pub created_at: DateTime<Utc>,
    pub prompt: String,
    pub input_type: InteractiveShellInputType,
    pub response_tx: Option<oneshot::Sender<PendingShellInputResolution>>,
}

impl std::fmt::Debug for PendingShellExecution {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PendingShellExecution")
            .field("execution_id", &self.execution_id)
            .field("session_id", &self.session_id)
            .field("executable_command", &self.executable_command)
            .field("display_command", &self.display_command)
            .field("run_mode", &self.run_mode)
            .field("timeout", &self.timeout)
            .field("created_at", &self.created_at)
            .field("prompt", &self.prompt)
            .field("input_type", &self.input_type)
            .finish_non_exhaustive()
    }
}

/// Thread-safe storage for pending shell executions
/// Manages a map of execution_id -> PendingShellExecution
#[derive(Debug)]
pub struct PendingExecutions(Mutex<HashMap<String, PendingShellExecution>>);

impl Default for PendingExecutions {
    fn default() -> Self {
        Self::new()
    }
}

impl PendingExecutions {
    pub fn new() -> Self {
        Self(Mutex::new(HashMap::new()))
    }

    pub fn insert(&self, exec: PendingShellExecution) {
        self.0
            .lock()
            .unwrap()
            .insert(exec.execution_id.clone(), exec);
    }

    pub fn remove(&self, id: &str) -> Option<PendingShellExecution> {
        self.0.lock().unwrap().remove(id)
    }

    pub fn remove_if_session_matches(
        &self,
        id: &str,
        session_id: &str,
    ) -> Result<Option<PendingShellExecution>, PendingExecutionLookupError> {
        let mut map = self.0.lock().unwrap();
        match map.get(id) {
            None => Ok(None),
            Some(pending) if pending.session_id != session_id => {
                Err(PendingExecutionLookupError::SessionMismatch)
            }
            Some(_) => Ok(map.remove(id)),
        }
    }

    pub fn remove_for_session(&self, session_id: &str) -> Vec<PendingShellExecution> {
        let mut map = self.0.lock().unwrap();
        let ids = map
            .iter()
            .filter(|(_, pending)| pending.session_id == session_id)
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();

        ids.into_iter()
            .filter_map(|id| map.remove(&id))
            .collect::<Vec<_>>()
    }

    /// Get count of pending executions (for monitoring)
    pub fn count(&self) -> usize {
        self.0.lock().unwrap().len()
    }

    /// Cleanup expired pending executions
    pub fn cleanup_expired(&self, ttl_seconds: u64) {
        let mut map = self.0.lock().unwrap();
        let now = chrono::Utc::now();
        let ttl_limit = i64::try_from(ttl_seconds).unwrap_or(i64::MAX);
        map.retain(|_, exec| {
            let age = now.signed_duration_since(exec.created_at);
            age.num_seconds() < ttl_limit
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pending_executions_cleanup() {
        let pending = PendingExecutions::new();
        let now = chrono::Utc::now();

        // Add one old entry (15 minutes ago)
        pending.insert(PendingShellExecution {
            execution_id: "old".to_string(),
            session_id: "sess".to_string(),
            executable_command: "ls".to_string(),
            display_command: "ls".to_string(),
            run_mode: "sync".to_string(),
            timeout: 30,
            created_at: now - chrono::Duration::minutes(15),
            prompt: "prompt".to_string(),
            input_type: InteractiveShellInputType::Text,
            response_tx: None,
        });

        // Add one new entry
        pending.insert(PendingShellExecution {
            execution_id: "new".to_string(),
            session_id: "sess".to_string(),
            executable_command: "ls".to_string(),
            display_command: "ls".to_string(),
            run_mode: "sync".to_string(),
            timeout: 30,
            created_at: now,
            prompt: "prompt".to_string(),
            input_type: InteractiveShellInputType::Text,
            response_tx: None,
        });

        assert_eq!(pending.count(), 2);

        // Cleanup entries older than 10 minutes (600s)
        pending.cleanup_expired(600);

        assert_eq!(pending.count(), 1);
        assert!(pending.remove("new").is_some());
        assert!(pending.remove("old").is_none());
    }
}
