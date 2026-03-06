So, I see multiple places where domain logic and DB logic leaks into the command layer.

1. **`assistant_crud_commands.rs`**: Mixes `get_assistant_repository()` with DB error mapping, looping over arrays for batch upserts, creating logic. Needs `AssistantService` extraction.
2. **`mcp_server_config_commands.rs`**: Mixes JSON parsing, validating name (`BuiltinServiceId`), verifying config (`McpServerService::verify_config`), saving to DB (`repo.create(...)`), and then updating cache (`repo.update_cached_tools(...)`). All this should be inside `McpServerService`.
3. **`scheduled_task_commands.rs`**: Calculates `next_run_at` using `compute_next_run`, calculates timestamps, manipulates DB. Needs `ScheduledTaskService`.
4. **`agent_commands.rs`**: The commands `agent_get_available_tools` and `agent_get_tools` access `manager` and delegate to it. The logic inside `AgentSessionManager::get_available_tools` and `AgentSessionManager::get_tools_for_session` handles configs and proxy status, which seems fine for `AgentSessionManager` (as it acts as a facade), but wait.
   The prompt mentions:
   "Extract Services: Move logic from `command_save_file` into a pure `FileService::save()` module. Adapter Pattern: Create adapters that translate raw API/DB data into clean Domain Objects before passing them to the UI. Lift State / Invert Control: Pass dependencies as arguments rather than instantiating them deep inside functions."
   Let's see where things instantiate deep inside functions.
   In `agent_service.rs` we have things like `get_session_manager()`.
   In `playbook_commands.rs` we have `PlaybookService::get_assistant_id_from_session` called from the command before calling `repo.create_playbook`. The command orchestrates the service and the repo! It should call `PlaybookService::create_playbook` instead.

I will focus on extracting services to remove "God Modules" and fat commands.

1. Create `AssistantService` in `src-tauri/src/services/assistant_service.rs`.
   - Move `create_assistant`, `update_assistant`, `delete_assistant`, `list_assistants`, `get_assistant`, `batch_upsert_assistants` logic into it.
2. Move `mcp_server_config_commands` logic into `McpServerService`.
   - Add `create_server_config`, `update_server_config`, `delete_server_config`, `list_server_configs`.
3. Move `playbook_commands` logic into `PlaybookService`.
   - Add `create_playbook`, `update_playbook`, `delete_playbook`, `get_playbook`, `toggle_playbook_bookmark`.
4. Create `ScheduledTaskService` in `src-tauri/src/services/scheduled_task_service.rs`.
   - Add `create_scheduled_task`, `list_scheduled_tasks`, `get_scheduled_task`, `update_scheduled_task`, `toggle_scheduled_task`, `delete_scheduled_task`.
