# Implementation Map

Use this map to find the main code surfaces for the teamwork refactor.

## Workspace constitution and scaffold

- `src-tauri/bundled_skills/task-force-builder/SKILL.md`
- `src-tauri/bundled_skills/task-force-builder/scripts/init_task_force.py`
- `src-tauri/bundled_skills/task-force-builder/references/coordination-contract.md`
- `src-tauri/src/agent/llm/prompt.rs`

What is missing:

- `agents.md` generation in the scaffold
- writing the original user request and teamwork base information into the scaffold
- stronger constitution guidance in the meta-skill

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
- `src-tauri/src/server/handlers/sessions.rs`

What is missing:

- explicit org-oriented lineage metadata and UX
- dedicated Org view
- keeping org separate from scheduled task groups

## Validation direction

Use existing repo validation and add targeted integration coverage where behavior changes:

- `pnpm lint`
- `pnpm build`
- `pnpm test:run -- --reporter=dot`
- `cargo check --tests --manifest-path src-tauri/Cargo.toml`

Add Rust integration tests under `src-tauri/tests/` for backend behavior changes.
