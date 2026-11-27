//! Handler selection and execution: matcher filtering, the stdio handler,
//! and the HTTP handler. Side effects (spawning, I/O) stay in this layer.

use super::command::{build_hook_command, HookEnv};
use super::http::run_http_hook;
use crate::services::hooks::context::append_hook_context;
use crate::services::hooks::output::{normalize, HookOutput};
use crate::services::hooks::schema::{HookEvent, HookHandler, DEFAULT_HOOK_TIMEOUT_MS};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::time::timeout;

pub(super) async fn run_matching_handler(
    session_id: &str,
    event: HookEvent,
    env: &HookEnv,
    handler: &HookHandler,
    envelope_json: &str,
) -> Option<HookOutput> {
    match run_hook_handler(event, env, handler, envelope_json).await {
        HookRunOutcome::Applied(output) => {
            if event == HookEvent::SessionStart {
                if let Some(ctx) = output
                    .additional_context
                    .as_ref()
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                {
                    append_hook_context(session_id, ctx);
                }
            }
            Some(output)
        }
        HookRunOutcome::Skipped(reason) => {
            log::debug!(
                "hook handler ({}) skipped for session {}: {}",
                event.as_str(),
                session_id,
                reason
            );
            None
        }
    }
}

pub(super) enum HookRunOutcome {
    Applied(HookOutput),
    Skipped(String),
}

async fn run_hook_handler(
    event: HookEvent,
    env: &HookEnv,
    handler: &HookHandler,
    envelope_json: &str,
) -> HookRunOutcome {
    let timeout_ms = if handler.timeout_ms == 0 {
        DEFAULT_HOOK_TIMEOUT_MS
    } else {
        handler.timeout_ms
    };
    let deadline = Duration::from_millis(timeout_ms);

    if let Some(http) = &handler.http {
        return run_http_hook(event, http, deadline, envelope_json).await;
    }

    let mut child = match build_hook_command(env, handler) {
        Ok(mut cmd) => match cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                return HookRunOutcome::Skipped(format!("spawn failed: {e}"));
            }
        },
        Err(e) => return HookRunOutcome::Skipped(e),
    };

    let pid = child.id();
    let mut stdin = child.stdin.take();
    let mut stdout_pipe = child.stdout.take();
    let mut stderr_pipe = child.stderr.take();
    let envelope_bytes = envelope_json.as_bytes().to_vec();
    let run_future = async {
        use tokio::io::AsyncReadExt;

        let write_stdin = async {
            if let Some(mut pipe) = stdin.take() {
                if let Err(e) = pipe.write_all(&envelope_bytes).await {
                    if e.kind() != std::io::ErrorKind::BrokenPipe {
                        return Err(format!("stdin write failed: {e}"));
                    }
                }
            }
            Ok::<(), String>(())
        };
        let read_stdout = async {
            let mut buf = Vec::new();
            if let Some(mut pipe) = stdout_pipe.take() {
                pipe.read_to_end(&mut buf)
                    .await
                    .map_err(|e| format!("stdout read failed: {e}"))?;
            }
            Ok::<Vec<u8>, String>(buf)
        };
        let read_stderr = async {
            let mut buf = Vec::new();
            if let Some(mut pipe) = stderr_pipe.take() {
                pipe.read_to_end(&mut buf)
                    .await
                    .map_err(|e| format!("stderr read failed: {e}"))?;
            }
            Ok::<Vec<u8>, String>(buf)
        };

        let (stdin_res, stdout_res, stderr_res) =
            tokio::join!(write_stdin, read_stdout, read_stderr);
        stdin_res?;
        let stdout = stdout_res?;
        let stderr = stderr_res?;
        let status = child
            .wait()
            .await
            .map_err(|e| format!("wait failed: {e}"))?;
        Ok::<std::process::Output, String>(std::process::Output {
            status,
            stdout,
            stderr,
        })
    };

    let output = match timeout(deadline, run_future).await {
        Ok(Ok(out)) => out,
        Ok(Err(reason)) => return HookRunOutcome::Skipped(reason),
        Err(_) => {
            if let Some(pid) = pid {
                let _ = tokio::task::spawn_blocking(move || {
                    crate::utils::process::force_kill_process_tree(pid)
                })
                .await;
            }
            return HookRunOutcome::Skipped("hook timed out".to_string());
        }
    };

    let status = output.status;
    let code = status.code().unwrap_or(-1);
    if code == 2 {
        return HookRunOutcome::Skipped("hook exit 2 (fail-open)".to_string());
    }
    if !status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return HookRunOutcome::Skipped(format!("hook exit {code}: {}", stderr.trim()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed = normalize(&stdout, event);
    HookRunOutcome::Applied(parsed)
}
