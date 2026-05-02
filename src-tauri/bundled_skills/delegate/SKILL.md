---
name: delegate
description: Delegate work between LibrAgent AI agent sessions using sub-agent sessions. Use when an agent needs to spawn, brief, monitor, or troubleshoot a child session with `startSession`, `checkSession`, or `messageToSession`, especially when deciding whether the child really needs parent workspace state, workspace instructions, or workspace-scoped skills.
---

# Delegate

Treat sub-agent delegation as session orchestration, not magic inheritance.

A child session keeps lineage to its parent, but it does **not** automatically inherit the parent's workspace, workspace instruction files, or workspace-local skills. If the task depends on those, either restate them in the handoff, choose a different assistant, or avoid delegation.

Read `references/delegation-patterns.md` when you need concrete handoff templates, troubleshooting patterns, or a quick matrix for what the child session can actually see.

## Delegation Workflow

1. Decide whether delegation is appropriate.
2. Choose the child's effective context.
3. Prepare a handoff that includes everything the child actually needs.
4. Start the child session.
5. Monitor or steer it with follow-up messages.
6. Merge the result back into the parent flow.

## 1. Decide Whether to Delegate

Delegate when the work is bounded, parallelizable, or benefits from a specialized assistant.

Good delegation targets:

- focused research or code reading
- isolated implementation tasks with clear deliverables
- long-running analysis where the parent can keep working
- specialized assistants that have assistant-scoped skills the parent lacks

Avoid delegation when the child must rely on live parent-only state, such as:

- the parent's temporary workspace files
- the parent's workspace `agents.md` / `CLAUDE.md` instructions
- workspace-local `skills/` content from the parent's workspace
- any behavior that assumes random parent context is implicitly copied into the child

If the work depends on any of those, delegation is usually the wrong move unless you can explicitly recreate that context for the child.

## 2. Choose the Child's Effective Context

Assume these rules:

- `startSession` creates a new child session with lineage metadata, not a cloned runtime context.
- The child gets its own session workspace by default.
- Workspace instructions are loaded from the **child** workspace, not the parent workspace.
- Workspace-scoped skills are resolved from the **child** workspace, not the parent workspace.
- Assistant-scoped skills come from the assistant you choose for `agentId`.
- Global skills remain available to both parent and child.

If the parent is running inside a task-force workspace, check `.libragent/teamwork.json` before delegating:

- If `executionSubstrate.mode` is `"org"`, prefer `startSession(..., includeCurrentOrg=true)` so the child joins the org and inherits the parent effective workspace by default. Switch to `org` for org-specific operating rules.
- If `executionSubstrate.mode` is `"scheduled"`, the delegation is likely a scheduled wake-up. Follow `schedule` for group management instead of ad-hoc delegation.
- Treat the app-local teamwork artifact directory as the orchestration/constitution storage. If the child also needs to edit code in a repo, keep the session workspace semantics separate from the teamwork artifact path.

Important limitations:

- Do **not** assume `agent.md` exists. Workspace behavior instructions are loaded from the first non-empty file among `agents.md`, `AGENTS.md`, `CLAUDE.md`, and `GEMINI.md`.
- Persona / tone instructions are loaded separately from the first non-empty file among `.github/SOUL.md`, `SOUL.md`, `.github/soul.md`, and `soul.md`.
- Both persona and workspace instruction content are cached for the session lifetime until the stable prompt cache is invalidated.
- `startSession` can override the child workspace with `workspaceOverride`; files and prompt state still follow the child session.
- Default to each session's own workspace. Use `workspaceOverride` only when the child should work in the same effective workspace as the parent or another already-existing workspace.

## 3. Prepare the Handoff

Make the task self-contained. The child should not need to guess what the parent meant.

Always include:

- exact objective
- hard scope boundaries
- expected output format
- critical paths, identifiers, or commands
- any rules from the parent workspace instructions that the child must obey
- any skill-specific procedure that may not exist in the child workspace

If the child needs a specific skill workflow, prefer one of these approaches:

- choose an assistant whose assistant-scoped skills already contain that workflow
- rely on a global skill that both sessions can access
- copy the critical procedure into the task text if the workflow is short

Say "use the same workspace as this session" only when you intentionally started the child in that shared workspace.

## 4. Start and Manage the Child Session

Use the builtin agent tools deliberately:

- `list(type="configs")` to find the right assistant and prefer its returned ID
- `startSession(agentId="...", task="...", waitForResult=false)` when you have the ID
- `startSession(agentId="...", task="...", workspaceOverride="/absolute/path")` when the child must run in a shared existing workspace
- `checkSession(sessionId)` to poll
- `checkSession(sessionId, wait=true)` when you want to block until a terminal result
- `messageToSession(sessionId, message)` to correct course or provide more input
- `list(type="sessions")` to inspect delegated children of the current session

Default to `waitForResult=false` unless the parent truly has nothing useful to do while waiting.

## 5. Review the Result Like an Adult

Do not blindly trust the child.

After the child returns:

- verify whether it actually used the correct scope
- check whether missing workspace instructions or skills likely distorted the result
- inspect referenced files or commands before presenting the answer as final
- send a follow-up message if the child stopped short or misunderstood the objective

## 6. Troubleshooting Heuristics

If the child cannot find a file, a local skill, or a workspace rule, assume isolation first.

Common causes:

- the child is in a different workspace
- the child used a different assistant than intended
- the needed procedure lived only in the parent's workspace `skills/` directory
- the parent assumed files or rules would be copied without using `workspaceOverride` or explicit task text
- the parent changed `agents.md` mid-session and expected the existing session prompt cache to refresh automatically

When in doubt, restate the missing context explicitly or stop delegating that particular subtask.
