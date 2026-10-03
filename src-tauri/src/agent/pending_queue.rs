//! Durable FIFO waiting prompts: messages table + thin `pending_queue` index.
//!
//! Routing invariants:
//! 1. Idle / session-start user request → append onto the active message stack and
//!    start the workflow (`start_workflow`). Must not enter `pending_queue`.
//! 2. Busy / Queued / Provisioning / compaction-in-flight → enqueue into
//!    `pending_events` + durable index only (not the active stack). The workflow
//!    loop dequeues via `claim_all_pending_messages` at the start of each LLM turn.
//! 3. Workflow finish → if waiters remain, continue the loop (claim on next turn)
//!    rather than going Idle with an orphaned queue. Cancel may discard instead.
//! 4. Compaction settle → Preflight resumes completion (which claims pending);
//!    Manual with waiters on Idle/Paused starts a turn from the queue; Busy/Queued
//!    leave claiming to the existing workflow lifecycle.

pub mod claiming;
pub mod hydration;
pub mod operations;
pub mod restore;

pub use claiming::*;
pub use hydration::*;
pub use operations::*;
pub use restore::*;
