# Implementation Map

Use this map to find the main code surfaces for the teamwork refactor.

## Workspace constitution and scaffold

- `src-tauri/bundled_skills/teamwork/SKILL.md`
- `src-tauri/bundled_skills/teamwork/scripts/init_task_force.py`
- `src-tauri/bundled_skills/teamwork/references/coordination-contract.md`
- `src-tauri/src/agent/llm/prompt.rs`

What is missing:

- stronger `primary_artifact` naming per role (currently always `*_NOTES.md`)
- `schemaVersion` upgrade path for `teamwork.json`

## Scheduled task backend

- `src-tauri/src/entity/scheduled_task.rs`
- `src-tauri/src/services/scheduled_task_service.rs`
- `src-tauri/src/repositories/scheduled_task_repository.rs`
- `src-tauri/src/commands/scheduled_task_commands.rs`
- `src-tauri/src/mcp/builtin/scheduled_task/`

What is missing:

- scheduled task group metadata
- stronger provenance
- Settings-backed backend-enforced policy

## Scheduled task frontend

- `src/lib/backend/scheduled-tasks.ts`
- `src/features/scheduled-tasks/ScheduledTasksPage.tsx`
- `src/features/scheduled-tasks/components/ScheduledTaskModal.tsx`
- `src/features/scheduled-tasks/hooks/useScheduledTasks.ts`

What is missing:

- group-centric UX
- separation between personal tasks and scheduled groups
- governance settings surface

## Org lineage and session UI

- `src/models/agent.ts`
- `src/context/AgentSessionListContext.tsx`
- `src/features/agent/components/SessionHistoryPanel.tsx`
- `src/features/history/Org.tsx`
- `src/features/history/org-sessions.ts`
- `src-tauri/src/server/handlers/sessions.rs`
- `src-tauri/src/entity/session.rs`
- `src-tauri/src/repositories/session_repository.rs`
- `src-tauri/src/services/agent_service.rs`

What is missing:

- explicit org metadata (`org_id`, `org_name`, `org_root_session_id`)
- explicit org-aware creation path instead of lineage-only inference
- minimal org tool group (`createOrg`, `spawnOrgAgent`, optional `getOrg`)
- replacing the provisional lineage-filtered Org view with org-card / org-chart UX
- resume behavior routed to org root session
- keeping org separate from scheduled task groups

## Validation direction

Use existing repo validation and add targeted integration coverage where behavior changes:

- `pnpm lint`
- `pnpm build`
- `pnpm test:run -- --reporter=dot`
- `cargo check --tests --manifest-path src-tauri/Cargo.toml`

Add Rust integration tests under `src-tauri/tests/` for backend behavior changes.

Add frontend regressions for:

- grouped-vs-standalone scheduled task UX
- explicit-org-only org filtering
- root-session resume from org view
