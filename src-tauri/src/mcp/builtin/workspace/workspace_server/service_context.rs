use super::{clear_context_cache, WorkspaceServer};
use serde_json::Value;

impl WorkspaceServer {
    pub async fn get_service_context_internal(
        &self,
        options: Option<&Value>,
    ) -> crate::mcp::types::ServiceContext {
        use super::super::context;
        use crate::mcp::types::{ContextVolatility, ServiceContext};

        let session_id = if let Some(opts) = options {
            opts.get("session_id")
                .and_then(|v| v.as_str())
                .unwrap_or(&self.session_id)
                .to_string()
        } else {
            self.session_id.clone()
        };

        const CACHE_TTL_SECS: u64 = 5;
        if let Ok(guard) = self.context_cache.try_read() {
            if let Some((cached_prompt, last_update)) = guard.as_ref() {
                if last_update.elapsed().as_secs() < CACHE_TTL_SECS {
                    return ServiceContext::new(cached_prompt.clone())
                        .with_structured_state(serde_json::json!({
                            "cached": true,
                            "session_id": session_id
                        }))
                        .with_volatility(ContextVolatility::Volatile);
                }
            }
        }

        let mut context_prompt = context::build_context_prompt(
            &session_id,
            &self.session_manager,
            &self.process_registry,
            &self.shell_manager,
        )
        .await;

        let live_state = context::build_workspace_live_state(
            &session_id,
            &self.session_manager,
            &self.shell_manager,
        )
        .await;

        let workspace_dir = live_state.workspace_dir;
        let shell_cwd = live_state.shell_cwd;
        let platform = context::ExecutionPlatform::for_session(&session_id, live_state.is_docker);

        let count_processes = || async {
            let reg = self.process_registry.read().await;
            let running = reg
                .entries
                .values()
                .filter(|e| e.session_id == session_id)
                .filter(|e| super::super::terminal_manager::is_active_process_status(&e.status))
                .count();
            let total = reg
                .entries
                .values()
                .filter(|e| e.session_id == session_id)
                .count();
            (running, total)
        };

        let (mut running_count, mut total_count) = count_processes().await;
        let mut recent_finished_count =
            context::count_recently_finished_processes(&self.process_registry, &session_id).await;

        let prompt_lists_no_running = context_prompt.contains("- Running Processes: None");
        if running_count == 0 && !prompt_lists_no_running {
            context_prompt = context::build_context_prompt(
                &session_id,
                &self.session_manager,
                &self.process_registry,
                &self.shell_manager,
            )
            .await;
            (running_count, total_count) = count_processes().await;
            recent_finished_count =
                context::count_recently_finished_processes(&self.process_registry, &session_id)
                    .await;
        }

        let prompt_is_idle = context_prompt.contains("- Running Processes: None")
            && !context_prompt.contains("- Recently Finished:");
        if running_count == 0 && recent_finished_count == 0 && prompt_is_idle {
            let mut guard = self.context_cache.write().await;
            *guard = Some((context_prompt.clone(), std::time::Instant::now()));
        } else {
            clear_context_cache(&self.context_cache).await;
        }

        ServiceContext::new(context_prompt)
            .with_structured_state(serde_json::json!({
                "workspace_dir": workspace_dir,
                "shell_cwd": shell_cwd,
                "is_docker": live_state.is_docker,
                "platform": platform.to_structured_json(),
                "processes": {
                    "running": running_count,
                    "total": total_count,
                    "recent_finished": recent_finished_count,
                },
                "shell_active": !shell_cwd.is_empty(),
                "tools_count": Self::tools_static().len()
            }))
            .with_volatility(ContextVolatility::Volatile)
    }
}
