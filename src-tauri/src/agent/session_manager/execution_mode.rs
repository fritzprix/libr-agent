use super::AgentSessionManager;
use std::sync::atomic::Ordering;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionMode {
    Normal,
    Yolo,
    Unsafe,
}

impl ExecutionMode {
    pub fn runtime_flags(self) -> (bool, bool) {
        match self {
            Self::Normal => (false, false),
            Self::Yolo => (true, false),
            Self::Unsafe => (false, true),
        }
    }

    pub fn include_hard_approvals(self) -> Option<bool> {
        match self {
            Self::Normal => None,
            Self::Yolo => Some(false),
            Self::Unsafe => Some(true),
        }
    }
}

impl std::str::FromStr for ExecutionMode {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "normal" => Ok(Self::Normal),
            "yolo" => Ok(Self::Yolo),
            "unsafe" => Ok(Self::Unsafe),
            _ => Err(format!("Unknown execution mode: {}", value)),
        }
    }
}

pub async fn set_execution_mode(
    manager: &AgentSessionManager,
    session_id: &str,
    mode: ExecutionMode,
) -> Result<(), String> {
    let (yolo_enabled, unsafe_enabled) = mode.runtime_flags();
    manager.session_repo
        .update_execution_mode(session_id, yolo_enabled, unsafe_enabled)
        .await
        .map_err(|e| format!("Failed to update session execution mode: {}", e))?;

    let has_active_session = {
        let active = manager.active_sessions.read().await;
        if let Some(session) = active.get(session_id) {
            session.yolo_mode.store(yolo_enabled, Ordering::SeqCst);
            session.unsafe_mode.store(unsafe_enabled, Ordering::SeqCst);
            true
        } else {
            false
        }
    };

    if !has_active_session {
        log::info!(
            "Updated execution mode for persisted session '{}' without active runtime state",
            session_id
        );
    }

    if let Some(include_hard_approvals) = mode.include_hard_approvals() {
        if has_active_session {
            super::approvals::approve_all_pending_tool_approvals(
                manager,
                session_id,
                include_hard_approvals,
            )
            .await?;
        }
    }

    Ok(())
}
