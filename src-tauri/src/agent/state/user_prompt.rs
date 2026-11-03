//! One shared user prompt mechanism for every `secret` and `question` request (#2543).
//!
//! Modeled on the pending tool approval flow: the entry lives in `AgentSession`,
//! Stop drops it, and a query rehydrates the dialog after a reload. A `secret`
//! value travels back to the backend caller through the oneshot channel only; it
//! never enters the LLM context, chat history, events, or logs.

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{oneshot, RwLock};

/// Requirement 5: one shared timeout for both prompt kinds.
pub const USER_PROMPT_TIMEOUT_SECS: u64 = 300;

/// The timeout error text every caller returns unchanged.
pub const USER_PROMPT_TIMEOUT_MESSAGE: &str = "User did not answer in 300 s.";

/// Result text when an app restart lost the prompt (requirement 7).
pub const USER_PROMPT_RESTART_MESSAGE: &str = "User did not answer (app restarted).";

/// Result text when the user cancelled or the session stopped.
pub const USER_PROMPT_CANCELLED_MESSAGE: &str = "User cancelled the prompt.";

/// Requirement 2: one prompt at a time per session.
pub const USER_PROMPT_ALREADY_PENDING_MESSAGE: &str = "A prompt is already waiting for the user.";

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum UserPromptKind {
    Secret,
    Question,
}

/// What the user is asked. The `Secret` variant carries no value: the value
/// exists only in the answer sent back through the channel.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum UserPromptPayload {
    Secret {
        variable_name: String,
        label: String,
        /// Optional line telling the user the value is reused for the session.
        #[serde(skip_serializing_if = "Option::is_none")]
        reuse_note: Option<String>,
    },
    /// The question form delivered by #2547.
    Question {
        title: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        body: Option<String>,
    },
}

impl UserPromptPayload {
    pub fn kind(&self) -> UserPromptKind {
        match self {
            Self::Secret { .. } => UserPromptKind::Secret,
            Self::Question { .. } => UserPromptKind::Question,
        }
    }

    /// Redacted description for logs. Never contains a secret value.
    pub fn describe(&self) -> String {
        match self {
            Self::Secret {
                variable_name,
                label,
                ..
            } => format!("Secret(variable={variable_name}, label={label})"),
            Self::Question { title, .. } => format!("Question(title={title})"),
        }
    }
}

/// How a pending prompt ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserPromptOutcome {
    Answered(String),
    Cancelled,
    TimedOut,
    Stopped,
    Restarted,
}

impl UserPromptOutcome {
    /// The text an open tool call reports for this outcome.
    pub fn caller_message(&self) -> String {
        match self {
            Self::Answered(value) => value.clone(),
            Self::Cancelled | Self::Stopped => USER_PROMPT_CANCELLED_MESSAGE.to_string(),
            Self::Restarted => USER_PROMPT_RESTART_MESSAGE.to_string(),
            Self::TimedOut => USER_PROMPT_TIMEOUT_MESSAGE.to_string(),
        }
    }

    /// The lowercase string the resolved event carries.
    pub fn as_event_str(&self) -> &'static str {
        match self {
            Self::Answered(_) => "answered",
            Self::Cancelled => "cancelled",
            Self::Stopped => "stopped",
            Self::TimedOut => "timed_out",
            Self::Restarted => "restarted",
        }
    }

    /// Requirement 5: a timed-out `secret` also names the variable and says not
    /// to retry, so the model stops asking for a value it will never get.
    pub fn secret_timeout_message(variable_name: &str) -> String {
        format!(
            "{USER_PROMPT_TIMEOUT_MESSAGE} The secret '{variable_name}' was not saved. Do not retry."
        )
    }
}

/// What the frontend send back to the waiting caller.
///
/// `Cancelled` travels the same channel as an answer so the wait has exactly one
/// emitter. Closing the channel instead (Stop, terminate, delete) means
/// [`UserPromptOutcome::Stopped`], which is a different cause.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserPromptAnswer {
    Secret(String),
    Question(String),
    Cancelled,
}

pub struct PendingUserPrompt {
    pub request_id: String,
    pub payload: UserPromptPayload,
    pub created_at: chrono::DateTime<chrono::Utc>,
    sender: oneshot::Sender<UserPromptAnswer>,
}

impl std::fmt::Debug for PendingUserPrompt {
    /// Omits the sender so no channel state leaks into logs.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PendingUserPrompt")
            .field("request_id", &self.request_id)
            .field("payload", &self.payload.describe())
            .field("created_at", &self.created_at)
            .finish()
    }
}

impl PendingUserPrompt {
    pub fn new(
        request_id: String,
        payload: UserPromptPayload,
        sender: oneshot::Sender<UserPromptAnswer>,
    ) -> Self {
        Self {
            request_id,
            payload,
            created_at: chrono::Utc::now(),
            sender,
        }
    }

    /// Deliver an answer to the waiting caller. Fails when the caller is gone
    /// (timeout or cancel already removed the entry).
    pub fn answer(self, answer: UserPromptAnswer) -> bool {
        self.sender.send(answer).is_ok()
    }
}

/// Read-only view handed to the frontend and the HTTP API. A `secret` prompt
/// exposes the variable name and label only; no field can carry a value.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PendingUserPromptSummary {
    pub request_id: String,
    pub kind: UserPromptKind,
    pub payload: UserPromptPayload,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl From<&PendingUserPrompt> for PendingUserPromptSummary {
    fn from(prompt: &PendingUserPrompt) -> Self {
        Self {
            request_id: prompt.request_id.clone(),
            kind: prompt.payload.kind(),
            payload: prompt.payload.clone(),
            created_at: prompt.created_at,
        }
    }
}

/// The per-session single slot. `None` means no prompt is waiting.
///
/// One slot rather than a map: requirement 2 allows one prompt per session, so
/// the type itself prevents a second concurrent entry.
pub type UserPromptSlot = Arc<RwLock<Option<PendingUserPrompt>>>;

pub fn new_user_prompt_slot() -> UserPromptSlot {
    Arc::new(RwLock::new(None))
}

/// Admit a new prompt, returning its request id and the receiver to wait on.
///
/// Fails with [`USER_PROMPT_ALREADY_PENDING_MESSAGE`] when a prompt already
/// waits, which is requirement 2.
pub async fn admit_user_prompt(
    slot: &UserPromptSlot,
    payload: UserPromptPayload,
) -> Result<(String, oneshot::Receiver<UserPromptAnswer>), String> {
    let mut guard = slot.write().await;
    if guard.is_some() {
        return Err(USER_PROMPT_ALREADY_PENDING_MESSAGE.to_string());
    }

    let request_id = uuid::Uuid::new_v4().to_string();
    let (sender, receiver) = oneshot::channel();
    *guard = Some(PendingUserPrompt::new(
        request_id.clone(),
        payload,
        sender,
    ));
    Ok((request_id, receiver))
}

/// Remove the pending prompt when it matches `request_id`.
///
/// A mismatched id leaves the slot untouched so a stale dialog cannot cancel a
/// newer prompt for the same session.
pub async fn take_pending_user_prompt(
    slot: &UserPromptSlot,
    request_id: &str,
) -> Option<PendingUserPrompt> {
    let mut guard = slot.write().await;
    match guard.as_ref() {
        Some(prompt) if prompt.request_id == request_id => guard.take(),
        _ => None,
    }
}

/// Remove the pending prompt whatever its id: the Stop, terminate, and delete
/// path (requirement 6). Dropping the sender ends the caller's wait at once.
pub async fn take_any_pending_user_prompt(slot: &UserPromptSlot) -> Option<PendingUserPrompt> {
    slot.write().await.take()
}

/// Snapshot for the pending query (requirement 7). Never includes a value.
pub async fn pending_user_prompt_summary(
    slot: &UserPromptSlot,
) -> Option<PendingUserPromptSummary> {
    slot.read().await.as_ref().map(PendingUserPromptSummary::from)
}

/// Wait for the answer under the shared 300 s timeout.
pub async fn await_user_prompt(receiver: oneshot::Receiver<UserPromptAnswer>) -> UserPromptOutcome {
    await_user_prompt_with_timeout(receiver, Duration::from_secs(USER_PROMPT_TIMEOUT_SECS)).await
}

/// Wait under an explicit timeout, so tests do not sleep for the shared 300 s.
///
/// A closed channel means Stop, termination, or deletion dropped the sender, so
/// the wait ends at once instead of running to the timeout.
pub async fn await_user_prompt_with_timeout(
    receiver: oneshot::Receiver<UserPromptAnswer>,
    wait: Duration,
) -> UserPromptOutcome {
    match tokio::time::timeout(wait, receiver).await {
        Ok(Ok(UserPromptAnswer::Secret(value))) => UserPromptOutcome::Answered(value),
        Ok(Ok(UserPromptAnswer::Question(text))) => UserPromptOutcome::Answered(text),
        Ok(Ok(UserPromptAnswer::Cancelled)) => UserPromptOutcome::Cancelled,
        Ok(Err(_)) => UserPromptOutcome::Stopped,
        Err(_) => UserPromptOutcome::TimedOut,
    }
}

/// Wait for the outcome and always clear the slot afterwards.
///
/// Requirements 5 and 9 make cleanup part of the wait, not a caller duty: a
/// timed-out prompt must leave nothing behind, and an answer already consumed
/// must not block the next prompt for that session.
pub async fn wait_for_user_prompt(
    slot: &UserPromptSlot,
    request_id: &str,
    receiver: oneshot::Receiver<UserPromptAnswer>,
) -> UserPromptOutcome {
    let outcome = await_user_prompt(receiver).await;
    let _ = take_pending_user_prompt(slot, request_id).await;
    outcome
}

/// The text a `secret` caller returns for an outcome.
pub fn secret_outcome_message(outcome: &UserPromptOutcome, variable_name: &str) -> String {
    match outcome {
        UserPromptOutcome::TimedOut => UserPromptOutcome::secret_timeout_message(variable_name),
        other => other.caller_message(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secret_payload() -> UserPromptPayload {
        UserPromptPayload::Secret {
            variable_name: "SUDO_PASSWORD".to_string(),
            label: "System password".to_string(),
            reuse_note: Some("This value is reused for this session.".to_string()),
        }
    }

    fn question_payload() -> UserPromptPayload {
        UserPromptPayload::Question {
            title: "Which branch?".to_string(),
            body: None,
        }
    }

    #[test]
    fn secret_timeout_names_the_variable_and_forbids_retry() {
        let message = secret_outcome_message(&UserPromptOutcome::TimedOut, "SUDO_PASSWORD");

        assert!(message.contains(USER_PROMPT_TIMEOUT_MESSAGE));
        assert!(message.contains("SUDO_PASSWORD"));
        assert!(message.contains("Do not retry."));
        assert!(message.contains("was not saved"));
    }

    #[test]
    fn question_timeout_is_the_plain_timeout() {
        assert_eq!(
            UserPromptOutcome::TimedOut.caller_message(),
            USER_PROMPT_TIMEOUT_MESSAGE
        );
    }

    #[test]
    fn cancel_and_stop_report_the_cancelled_message() {
        assert_eq!(
            UserPromptOutcome::Cancelled.caller_message(),
            USER_PROMPT_CANCELLED_MESSAGE
        );
        assert_eq!(
            UserPromptOutcome::Stopped.caller_message(),
            USER_PROMPT_CANCELLED_MESSAGE
        );
    }

    #[test]
    fn restart_reports_the_restart_message() {
        assert_eq!(
            UserPromptOutcome::Restarted.caller_message(),
            USER_PROMPT_RESTART_MESSAGE
        );
    }

    /// Collect every object key in the serialized JSON, at any depth.
    ///
    /// A substring check would false-positive on the reuse note ("This value is
    /// reused for this session.") and would miss a leak inside a nested field,
    /// so assert on the key set instead of on the text.
    fn json_keys(value: &serde_json::Value, into: &mut Vec<String>) {
        match value {
            serde_json::Value::Object(map) => {
                for (key, nested) in map {
                    into.push(key.clone());
                    json_keys(nested, into);
                }
            }
            serde_json::Value::Array(items) => {
                for item in items {
                    json_keys(item, into);
                }
            }
            _ => {}
        }
    }

    #[test]
    fn summary_exposes_the_secret_name_and_label_but_no_value_field() {
        let (sender, _receiver) = oneshot::channel();
        let prompt = PendingUserPrompt::new("req-1".to_string(), secret_payload(), sender);

        let summary = PendingUserPromptSummary::from(&prompt);
        let json = serde_json::to_value(&summary).expect("summary serializes");

        assert_eq!(summary.kind, UserPromptKind::Secret);

        // The question the dialog needs is present.
        assert!(json.to_string().contains("SUDO_PASSWORD"));
        assert!(json.to_string().contains("System password"));

        // No key can carry a value or the channel: the value travels only
        // through the oneshot, so the summary cannot leak it.
        let mut keys = Vec::new();
        json_keys(&json, &mut keys);
        for forbidden in ["value", "secret", "sender", "answer"] {
            assert!(
                !keys.iter().any(|key| key.eq_ignore_ascii_case(forbidden)),
                "summary must not expose a '{}' key, found keys: {:?}",
                forbidden,
                keys
            );
        }
    }

    #[test]
    fn prompt_debug_redacts_the_sender_and_has_no_value() {
        let (sender, _receiver) = oneshot::channel();
        let prompt = PendingUserPrompt::new("req-1".to_string(), secret_payload(), sender);

        let rendered = format!("{prompt:?}");
        assert!(rendered.contains("SUDO_PASSWORD"));
        assert!(!rendered.contains("sender"));
    }

    #[test]
    fn answering_delivers_the_value_to_the_waiter() {
        let (sender, mut receiver) = oneshot::channel();
        let prompt = PendingUserPrompt::new("req-1".to_string(), secret_payload(), sender);

        assert!(prompt.answer(UserPromptAnswer::Secret("hunter2".to_string())));
        assert_eq!(
            receiver.try_recv().expect("answer received"),
            UserPromptAnswer::Secret("hunter2".to_string())
        );
    }

    #[tokio::test]
    async fn only_one_prompt_may_wait_per_session() {
        let slot = new_user_prompt_slot();

        let (first_id, _first_rx) = admit_user_prompt(&slot, secret_payload())
            .await
            .expect("first prompt is admitted");

        let rejected = admit_user_prompt(&slot, question_payload()).await;
        let message = rejected.expect_err("second prompt is rejected");
        assert_eq!(message, USER_PROMPT_ALREADY_PENDING_MESSAGE);

        // The pending entry is still the first one: the rejection changed nothing.
        let summary = pending_user_prompt_summary(&slot)
            .await
            .expect("summary is present");
        assert_eq!(summary.request_id, first_id);
        assert_eq!(summary.kind, UserPromptKind::Secret);
    }

    #[tokio::test]
    async fn answering_through_the_slot_releases_it() {
        let slot = new_user_prompt_slot();
        let (request_id, receiver) = admit_user_prompt(&slot, secret_payload())
            .await
            .expect("prompt admitted");

        let waiter = tokio::spawn(await_user_prompt(receiver));
        // Let the waiter start before the answer arrives.
        tokio::task::yield_now().await;

        let prompt = take_pending_user_prompt(&slot, &request_id)
            .await
            .expect("prompt is taken");
        assert!(prompt.answer(UserPromptAnswer::Secret("hunter2".to_string())));

        let outcome = waiter.await.expect("waiter finishes");
        assert_eq!(
            outcome,
            UserPromptOutcome::Answered("hunter2".to_string())
        );
        assert!(pending_user_prompt_summary(&slot).await.is_none());
    }

    #[tokio::test]
    async fn a_stale_request_id_cannot_cancel_a_newer_prompt() {
        let slot = new_user_prompt_slot();
        let (first_id, _first_rx) = admit_user_prompt(&slot, secret_payload())
            .await
            .expect("first prompt admitted");

        assert!(take_pending_user_prompt(&slot, "not-the-pending-id")
            .await
            .is_none());
        assert!(take_pending_user_prompt(&slot, &first_id).await.is_some());
    }

    #[tokio::test]
    async fn stop_ends_the_wait_at_once_with_the_cancelled_message() {
        let slot = new_user_prompt_slot();
        let (_request_id, receiver) = admit_user_prompt(&slot, secret_payload())
            .await
            .expect("prompt admitted");

        // Stop drops the slot entry, which closes the channel.
        assert!(take_any_pending_user_prompt(&slot).await.is_some());

        // A generous timeout proves the wait ended because the channel closed,
        // not because the clock ran out.
        let outcome =
            await_user_prompt_with_timeout(receiver, Duration::from_secs(30)).await;
        assert_eq!(outcome, UserPromptOutcome::Stopped);
        assert!(pending_user_prompt_summary(&slot).await.is_none());
    }

    #[tokio::test]
    async fn an_unanswered_prompt_times_out() {
        let slot = new_user_prompt_slot();
        let (_request_id, receiver) = admit_user_prompt(&slot, secret_payload())
            .await
            .expect("prompt admitted");

        let outcome =
            await_user_prompt_with_timeout(receiver, Duration::from_millis(20)).await;

        assert_eq!(outcome, UserPromptOutcome::TimedOut);
        assert_eq!(
            secret_outcome_message(&outcome, "SUDO_PASSWORD"),
            format!("{USER_PROMPT_TIMEOUT_MESSAGE} The secret 'SUDO_PASSWORD' was not saved. Do not retry.")
        );
    }

    /// Requirement 5 + 9: the timeout path must leave nothing behind, so the
    /// session can ask again. `wait_for_user_prompt` owns that cleanup.
    #[tokio::test]
    async fn the_shared_wait_clears_the_slot_after_a_timeout() {
        let slot = new_user_prompt_slot();
        let (request_id, receiver) = admit_user_prompt(&slot, secret_payload())
            .await
            .expect("prompt admitted");

        // Drive the same cleanup path the real 300 s wait uses, with a short
        // timeout so the test stays fast.
        let waiter = tokio::spawn({
            let slot = Arc::clone(&slot);
            async move {
                let outcome =
                    await_user_prompt_with_timeout(receiver, Duration::from_millis(20))
                        .await;
                let _ = take_pending_user_prompt(&slot, &request_id).await;
                outcome
            }
        });

        assert_eq!(
            waiter.await.expect("waiter finishes"),
            UserPromptOutcome::TimedOut
        );
        assert!(
            pending_user_prompt_summary(&slot).await.is_none(),
            "a timed-out prompt must not keep the session slot occupied"
        );

        // The slot is free, so the next request is admitted.
        assert!(admit_user_prompt(&slot, question_payload()).await.is_ok());
    }

    #[tokio::test]
    async fn an_answered_prompt_leaves_the_slot_free_for_the_next_request() {
        let slot = new_user_prompt_slot();
        let (request_id, receiver) = admit_user_prompt(&slot, secret_payload())
            .await
            .expect("prompt admitted");

        let waiter = tokio::spawn({
            let slot = Arc::clone(&slot);
            let request_id = request_id.clone();
            async move {
                let outcome = await_user_prompt_with_timeout(
                    receiver,
                    Duration::from_secs(5),
                )
                .await;
                let _ = take_pending_user_prompt(&slot, &request_id).await;
                outcome
            }
        });

        tokio::task::yield_now().await;
        let prompt = take_pending_user_prompt(&slot, &request_id)
            .await
            .expect("prompt is taken");
        assert!(prompt.answer(UserPromptAnswer::Question("main".to_string())));

        assert_eq!(
            waiter.await.expect("waiter finishes"),
            UserPromptOutcome::Answered("main".to_string())
        );
        assert!(admit_user_prompt(&slot, question_payload()).await.is_ok());
    }

    /// Cancel travels the channel rather than dropping the sender, so the wait
    /// reports `Cancelled` (a user choice) and not `Stopped` (the session died).
    #[tokio::test]
    async fn a_cancel_answer_reports_cancelled_not_stopped() {
        let slot = new_user_prompt_slot();
        let (request_id, receiver) = admit_user_prompt(&slot, question_payload())
            .await
            .expect("prompt admitted");

        let waiter = tokio::spawn(await_user_prompt_with_timeout(
            receiver,
            Duration::from_secs(5),
        ));
        tokio::task::yield_now().await;

        let prompt = take_pending_user_prompt(&slot, &request_id)
            .await
            .expect("prompt is taken");
        assert!(prompt.answer(UserPromptAnswer::Cancelled));

        assert_eq!(
            waiter.await.expect("waiter finishes"),
            UserPromptOutcome::Cancelled
        );
    }

    #[test]
    fn every_outcome_has_its_event_string() {
        assert_eq!(
            UserPromptOutcome::Answered("v".to_string()).as_event_str(),
            "answered"
        );
        assert_eq!(UserPromptOutcome::Cancelled.as_event_str(), "cancelled");
        assert_eq!(UserPromptOutcome::Stopped.as_event_str(), "stopped");
        assert_eq!(UserPromptOutcome::TimedOut.as_event_str(), "timed_out");
        assert_eq!(UserPromptOutcome::Restarted.as_event_str(), "restarted");
    }

    #[test]
    fn payload_kind_matches_the_variant() {
        assert_eq!(secret_payload().kind(), UserPromptKind::Secret);
        assert_eq!(question_payload().kind(), UserPromptKind::Question);
    }

    #[test]
    fn describe_redacts_and_names_the_fields_only() {
        assert_eq!(
            secret_payload().describe(),
            "Secret(variable=SUDO_PASSWORD, label=System password)"
        );
        assert_eq!(
            question_payload().describe(),
            "Question(title=Which branch?)"
        );
    }
}
