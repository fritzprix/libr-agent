# Session / workspace isolation

## Default child (`agent__startSession`)

| Asset | Parent | Child (default) |
| --- | --- | --- |
| Session workspace | Parent dir | **New isolated** workspace |
| `agents.md` / `SOUL.md` | Parent workspace | Loaded from **child** workspace (often empty) |
| Workspace-local skills | Parent workspace | Child workspace only |
| Assistant-scoped skills | Caller's assistant | Child's `agentId` assistant |
| Global / system skills | Shared | Shared |
| Scratchpad | Parent session | **Separate**; parent cannot read child notes |

After `agent__checkSession`, read Metadata `workspace:`:

- `SHARED with caller` — relative paths in the shared root are safe
- `ISOLATED` — use absolute paths from Metadata or rely on Result text

## Org children

When teamwork `executionSubstrate.mode` is `"org"`, org-visible children typically **share the org root effective workspace**, so they share the same `agents.md` / `SOUL.md` unless `workspaceOverride` points elsewhere.

Prefer **org** / **teamwork** skills for durable multi-agent constitution; prefer **delegate** for one-off bounded handoffs.

## Handoff rule

Assume the child knows nothing the parent did not put in the task text (or in a shared workspace the child can actually see). Require deliverables in the child's **final text response** for parent recovery (`waitForResult` / checkSession Result).
