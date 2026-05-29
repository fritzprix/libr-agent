1. *Modify backend to include settings deletion during reset*
   - Use `replace_with_git_merge_diff` to modify `src-tauri/src/services/agent_service/reset.rs`.
   - Update the reset logic to retrieve the `SettingsRepository` using `crate::state::try_get_settings_repository()`.
   - Use `settings_repo.list().await` to get all settings (which returns `Result<Vec<crate::entity::settings::Model>, DbError>`), and `settings_repo.delete(&setting.key).await` to delete them.
   - Code changes:
   ```rust
<<<<<<< SEARCH
        let mcp_repo = get_mcp_server_repository();
        let servers = mcp_repo
            .list()
            .await
            .map_err(|e| format!("Failed to list MCP servers: {}", e))?;
        for server in servers {
            mcp_repo
                .delete(&server.name)
                .await
                .map_err(|e| format!("Failed to delete MCP server {}: {}", server.name, e))?;
        }

        if let Err(error) = crate::services::assistant_init::ensure_default_assistants().await {
=======
        let mcp_repo = get_mcp_server_repository();
        let servers = mcp_repo
            .list()
            .await
            .map_err(|e| format!("Failed to list MCP servers: {}", e))?;
        for server in servers {
            mcp_repo
                .delete(&server.name)
                .await
                .map_err(|e| format!("Failed to delete MCP server {}: {}", server.name, e))?;
        }

        if let Some(settings_repo) = crate::state::try_get_settings_repository() {
            if let Ok(settings) = settings_repo.list().await {
                for setting in settings {
                    let _ = settings_repo.delete(&setting.key).await;
                }
            }
        }

        if let Err(error) = crate::services::assistant_init::ensure_default_assistants().await {
>>>>>>> REPLACE
   ```
2. *Verify backend changes*
   - Use `run_in_bash_session` to execute `git diff src-tauri/src/services/agent_service/reset.rs` to verify the changes were applied correctly.
3. *Modify React component to remove N+1 IPC calls*
   - Use `replace_with_git_merge_diff` to modify `src/features/settings/hooks/useSettingsPageController.ts`.
   - Remove the `try...catch` block around `dbUtils.clearAllObjects()`, `dbUtils.clearAllSessions()`, `dbUtils.clearAllAssistants()`, `dbUtils.clearAllMCPServers()`, and `dbUtils.clearAllPlaybooks()`. The function will now only set `setIsResetting(true)`, await `backendFactoryReset()`, and show the success/error toast.
   - Code changes:
   ```javascript
<<<<<<< SEARCH
  const handleFactoryReset = useCallback(async () => {
    setIsResetting(true);
    try {
      try {
        await dbUtils.clearAllObjects();
        await dbUtils.clearAllSessions();
        await dbUtils.clearAllAssistants();
        await dbUtils.clearAllMCPServers();
        await dbUtils.clearAllPlaybooks();
      } catch (error) {
        logger.error('Failed to clear frontend DB during factory reset', error);
      }

      await backendFactoryReset();

      toast.success(
=======
  const handleFactoryReset = useCallback(async () => {
    setIsResetting(true);
    try {
      await backendFactoryReset();

      toast.success(
>>>>>>> REPLACE
   ```
4. *Verify frontend changes*
   - Use `run_in_bash_session` to execute `git diff src/features/settings/hooks/useSettingsPageController.ts` to verify the changes were applied correctly.
5. *Create Journal Entry*
   - Use `run_in_bash_session` to execute `cat << 'INNER_EOF' > .jules/airlock.md
## 2025-05-29 - Batched IPC Operations
**Learning:** Bulk operations driven from React (like looping through all items to delete them) create severe N+1 IPC call patterns that choke the Tauri bridge.
**Action:** Always use or create batched Tauri commands for bulk operations (e.g., clearing all state) instead of orchestrating the loop in the React frontend.
INNER_EOF`.
6. *Verify Journal Entry*
   - Use `run_in_bash_session` to execute `cat .jules/airlock.md` to confirm its contents were written correctly.
7. *Run linters and tests*
   - Execute the test suite and linters via `run_in_bash_session`: `pnpm lint`, `pnpm test run`, and `cargo check`.
8. *Complete pre-commit steps*
   - Complete pre-commit steps to ensure proper testing, verification, review, and reflection are done.
9. *Create a Pull Request*
   - Use `run_in_bash_session` to execute the following git and PR commands to submit the changes:
   ```bash
   git checkout -b airlock-factory-reset
   git add -f .jules/airlock.md
   git commit -am "🚪 Airlock: Refactor factory reset to backend"
   gh pr create --title "🚪 Airlock: Refactor factory reset to backend" --body "💡 What: Extracted settings deletion logic to the Rust backend and removed the individual object deletion loops in the React frontend.
🎯 Why: The React frontend was fetching all objects (settings, sessions, assistants, etc.) and deleting them one by one, creating severe N+1 IPC call bottlenecks. The backend \`factory_reset\` command now handles all deletions in a single batched operation.
📊 Impact: Reduces IPC chattiness during factory reset and enforces separation of concerns by keeping bulk database operations entirely in the backend.
🔬 Verification: Trigger a factory reset from the settings page and verify that all objects are deleted correctly and that no N+1 IPC calls are made."
   ```
