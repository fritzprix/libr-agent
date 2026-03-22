# Dynamic Agent Requirement Templates

Match these templates to the current `agent__list` to determine the "ideal" state of each agent based on its mission.

## Template: Orchestrator/Strategist
- **Mission**: Strategy, delegation, high-level planning.
- **Required Builtins**: `planning`, `assistant`, `session_api`, `workspace`.
- **Ideal Tool Categories**: `Search & Research`, `Financial & Economic Data`.

## Template: Field Agent/Researcher
- **Mission**: Information retrieval, deep dives, verification.
- **Required Builtins**: `browser`, `workspace`, `attachments`.
- **Ideal Tool Categories**: `Search & Research`, `AI & Multimedia`.

## Template: Developer/Engineer
- **Mission**: Implementation, refactoring, system setup.
- **Required Builtins**: `workspace`, `planning`, `session_api`.
- **Ideal Tool Categories**: `Technical & Engineering`.

## Template: Creative/Artisan
- **Mission**: Visual output, multimedia generation.
- **Required Builtins**: `ui`, `media`, `workspace`.
- **Ideal Tool Categories**: `AI & Multimedia`.

## Template: System Administrator
- **Mission**: Environment setup, tool management.
- **Required Builtins**: `mcp_manager`, `bootstrap`, `assistant`.
- **Ideal Tool Categories**: `Technical & Engineering` (especially for documentation).

## Evaluation Rule
1. Find the template that best matches the agent's `description`.
2. Compare current `builtinCapabilities` and `externalMcpServers` with the template's requirements.
3. Recommend additions/removals to align with the template.
