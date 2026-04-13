# scheduled_task builtin MCP server

This module exposes the existing scheduled-task backend to agents through a dedicated builtin MCP server.

## What is implemented

### Public MCP tools

The server currently exposes six agent-facing tools:

- `createScheduledTask`
- `listScheduledTasks`
- `getScheduledTask`
- `updateScheduledTask`
- `toggleScheduledTask`
- `deleteScheduledTask`

These tools are intentionally CRUD-shaped and map closely to the already-existing scheduled task service layer.

### Implemented module structure

```text
scheduled_task/
├── mod.rs
├── tools.rs
├── handlers.rs
├── formatting.rs
└── README.md
```

- `mod.rs` defines `ScheduledTaskServer`, static metadata, static tool exposure, service context, and tool dispatch.
- `tools.rs` defines the MCP tool schemas and descriptions.
- `handlers.rs` validates arguments, checks assistant/workspace inputs, calls the existing scheduled task service, and returns text-first MCP results.
- `formatting.rs` centralizes task detail/list rendering and JSON shaping.

### Wiring completed outside this directory

The new builtin server is already wired into the standard builtin MCP surfaces:

1. `src-tauri/src/mcp/builtin/mod.rs`
   - registers `pub mod scheduled_task;`
2. `src-tauri/src/mcp/builtin/service_id.rs`
   - adds `BuiltinServiceId::ScheduledTask`
   - supports aliases `scheduled_task` and `scheduled-task`
   - registers it as an **optional** builtin service
3. `src-tauri/src/mcp/service_proxy/factory.rs`
   - creates a per-session `ScheduledTaskServer`
4. `src-tauri/src/mcp/server/tools.rs`
   - exposes static metadata and static tool definitions
5. `src-tauri/src/mcp/builtin/error_guidance/*`
   - adds scheduled-task-specific recovery guidance
6. `src-tauri/tests/builtin_service_registry_tests.rs`
   - verifies registry presence
   - verifies static tool exposure

The generated frontend builtin-service list was also refreshed, so the new builtin appears in generated service metadata.

## Current behavior

### Why this server exists

Before this work, the backend already had:

- scheduled task persistence
- cron/timezone computation
- task runner logic
- pinned session reuse
- workspace override support

But agents had no builtin MCP surface to create or manage those tasks directly.

This server closes that gap.

### Design choices already made

#### 1. Dedicated server, not planning extension

`scheduled_task` is its own builtin server because scheduled tasks are:

- persistent
- cross-session
- scheduler-facing

That is a bad fit for the session-local `planning` domain.

#### 2. Optional builtin, not core

This service is registered as optional because it is powerful and easy to abuse.

That keeps it available for advanced assistants and Team Work style orchestration without silently handing recurring automation to every agent by default.

#### 3. Text-first responses

Task IDs are surfaced in visible text, not only JSON structured data.

That matters because agents act on text content. If IDs were hidden only in structured content, follow-up calls would be brittle and stupid.

#### 4. Thin MCP layer over existing services

The handlers are intentionally thin and reuse:

- `ScheduledTaskService`
- `ScheduledTaskRepository`
- assistant existence checks
- workspace override validation

This keeps persistence and schedule computation in the existing backend service layer instead of duplicating business logic inside the MCP server.

### Validation already present in handlers

The current implementation validates:

- malformed tool arguments
- assistant existence
- workspace override absolute path requirement
- restricted system path rejection
- workspace override directory existence/readability
- invalid cron / invalid timezone failures
- missing scheduled task IDs on get/update/toggle/delete

### Service context

The service context currently summarizes:

- total scheduled tasks
- enabled/disabled counts
- caller session ID
- up to three nearest enabled runs

It deliberately does **not** dump the full schedule set into the prompt.

## What is still missing

The core MCP layer exists, but the feature is **not product-complete**.

### 1. Governance and safety policy

This is the biggest remaining gap.

The current server allows mutation if the builtin is enabled. That is enough for engineering validation, but not enough for safe autonomous orchestration.

Still needed:

- approval flow for create/update/delete operations
- minimum cron interval guardrails
- quotas per assistant / workspace / session
- duplicate schedule detection
- ownership metadata for auditability
- clearer policy around which assistants may schedule which other assistants

Without those guardrails, agents can create recurring garbage surprisingly fast.

### 2. Better Team Work integration

This server is the missing backend primitive for Team Work, but the higher-level workflow is still unfinished.

Still needed:

- update `teamwork` guidance so it explicitly uses scheduled-task tools
- define safe recurring-team patterns
- teach bundled teamwork skills when **not** to schedule loops
- make the coordination contract and scheduler contract line up cleanly

### 3. UX and observability

The MCP surface exists, but users still need product-level visibility.

Still needed:

- UI surfacing for scheduled task provenance / ownership
- clearer display of task source assistant and target workspace
- task history / execution outcome visibility
- easy inspection of pinned session reuse behavior
- Team Work-oriented status surfaces rather than raw schedule rows

### 4. Stronger tests

Right now the new work has registry/static-surface coverage and full repository validation passed, but behavior coverage is still too thin.

Still needed under `src-tauri/tests/`:

- end-to-end create/list/get/update/toggle/delete integration tests
- invalid cron and invalid timezone behavior tests
- workspace override validation behavior tests
- assistant-not-found behavior tests
- service context shape tests
- optional-builtin enable/disable behavior tests at agent/runtime level

### 5. Mutation ergonomics

The tool layer works, but some ergonomics are still rough.

Possible improvements:

- partial update summaries in structured data with old/new values
- task filtering by workspace or status beyond `enabled`
- last run outcome and failure reason exposure
- better distinction between validation failures and operational failures
- stronger recovery hints for duplicate/near-identical schedules

### 6. Permission model decision

One product question is still open:

- should an agent be allowed to schedule any assistant by `assistantId`
- or only the assistant/session currently in control

The current implementation supports explicit `assistantId`, which is useful for Team Work orchestration, but it also expands blast radius. That needs an explicit product decision, not vibes.

## Suggested next implementation order

1. Add integration tests for actual scheduled task behavior.
2. Add schedule safety policy at the MCP boundary.
3. Wire `teamwork` to the new tools.
4. Add UI/observability for recurring teamwork.
5. Revisit permission policy for cross-assistant scheduling.

## Notes for future changes

- Keep responses text-first. Do not hide task IDs only in structured content.
- Keep this server thin. Business rules belong in shared service/repository layers where possible.
- Avoid turning service context into a giant dump; summarize and point agents to `listScheduledTasks()`.
- If stronger mutation safety is added, put it here at the MCP boundary so autonomous callers hit the policy consistently.
