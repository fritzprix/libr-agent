---
name: skill-deployer
description: "Guide for deploying a newly created skill to the correct scope within LibrAgent. Use this skill after creating a skill with skill-creator, when you need to install it so it becomes available to an agent. Triggers: deploy skill, install skill, save skill to [scope], make skill available, publish skill."
---

# Skill Deployer

This skill guides the process of installing a finished skill into the correct scope so it is actually loaded and used by agents.

## Scope Decision

Choose one scope based on intended reach:

| Scope | When to use | Override priority |
| --- | --- | --- |
| **workspace** | Skill is specific to the current project or session | Highest (overrides assistant and global) |
| **assistant** | Skill is for one specific assistant only | Middle (overrides global) |
| **global** | Skill is general-purpose and should be available everywhere | Lowest |

**Rule of thumb**: default to `workspace` when in doubt. It is the safest option — it cannot affect other sessions and is easy to clean up.

## Deployment Paths

### Global scope

```text
{skillsDirectory}/
└── {skill-name}/
    └── SKILL.md
```

`skillsDirectory` is set in Settings → System → Skills Directory.
Since the Tauri command `get_default_skills_directory` is not exposed to the agent as an MCP tool, do **NOT** attempt to call it directly. Instead:
1. **Inspect `<available_skills>`**: Read the system prompt's `<available_skills>` block. Look at the `<location>` of any existing global skill (e.g. `/home/alice/.local/share/libr-agent/skills/mcp-builder/SKILL.md`) and extract the parent directory (`/home/alice/.local/share/libr-agent/skills`).
2. **OS Standard Default Paths**: If no skills are listed, look for or check the existence of standard global skills directories:
   * **Linux**: `~/.local/share/libr-agent/skills/`
   * **macOS**: `~/Library/Application Support/libr-agent/skills/`
   * **Windows**: `%APPDATA%\libr-agent\skills\`
Example resolved path: `C:\Users\alice\AppData\Roaming\LibrAgent\skills\my-skill\SKILL.md`

### Assistant scope

```text
{dataDir}/assistants/{assistant-id}/skills/
└── {skill-name}/
    └── SKILL.md
```

Example: `C:\Users\alice\AppData\Roaming\LibrAgent\assistants\asst_abc123\skills\my-skill\SKILL.md`

### Workspace scope

```text
{workspace-root}/.libragent/skills/
└── {skill-name}/
    └── SKILL.md
```

Example: `/home/alice/project/.libragent/skills/my-skill/SKILL.md`
The workspace root is the session's working directory. It is visible in the system prompt under:

```text
## Workspace
**Workspace Root**: /home/alice/project
```

Read this value directly — no tool call needed.

> [!NOTE]
> The Rust backend automatically migrates legacy `{workspace-root}/skills` to `{workspace-root}/.libragent/skills` if it exists.

### Agent scope (Auto-discovered)

```text
{workspace-root}/{agent-directory}/skills/
└── {skill-name}/
    └── SKILL.md
```

LibrAgent automatically scans specific agent tool directories under the workspace root. You can deploy skills into these paths for local agent development:
*   `.agents/skills/` (LibrAgent local development directory)
*   `.gemini/skills/`
*   `.cursor/skills/`
*   `.copilot/skills/`
*   `.windsurf/skills/`
*   `.claude/skills/`
*   `.cline/skills/`
*   `.continue/skills/`

## Deployment Procedure

### Step 1 — Determine the target path

For **workspace** (most common):

```text
{workspace-root}/.libragent/skills/{skill-name}/SKILL.md
```

Read the `Workspace Root` from the active Workspace service context. Alternatively, deploy to an **agent import path** (e.g. `{workspace-root}/.agents/skills/{skill-name}/SKILL.md`) for IDE-specific agent development.

For **global**:

1. Retrieve the `skillsDirectory` path by analyzing `<available_skills>` location paths in the system prompt, or falling back to checking standard OS default paths.
2. Append `/{skill-name}/SKILL.md`.

For **assistant**:

1. The user must provide the assistant's ID (visible in the Assistants settings panel).
2. Construct: `{dataDir}/assistants/{assistant-id}/skills/{skill-name}/SKILL.md`

### Step 2 — Write the SKILL.md

Use `createFile` (workspace tool) with the full resolved path and the SKILL.md content. If the skill contains additional files (`scripts/`, `references/`), write each file in turn.

```text
createFile(
  path: "{target-path}/SKILL.md",
  content: "---\nname: ...\n..."
)
```

### Step 3 — Verify deployment

After writing, confirm the skill is discoverable by checking the system prompt on the **next agent turn**. The skills server injects an `<available_skills>` XML block into the system prompt. Each entry looks like this:

```xml
<skill source="workspace">
  <name>my-new-skill</name>
  <description>...</description>
  <location>/absolute/path/to/skills/my-new-skill/SKILL.md</location>
</skill>
```

Check that:

1. The skill name appears in `<available_skills>`.
2. The `source` attribute matches the intended scope (`"workspace"`, `"assistant"`, or `"global"`).
3. The `<location>` path points to the file you just wrote.

Alternatively, inspect the target folder contents using `listDirectory` to verify the files are physically present on disk.

### Step 4 — Announce the result

Report:

- The full path written
- The scope chosen
- The skill name as it will appear in the system prompt

## Bundled Resources

If the skill includes scripts, references, or assets, write them relative to the SKILL.md:

```text
createFile("{target-path}/scripts/run.sh", ...)
createFile("{target-path}/references/api-reference.md", ...)
```

## Coalesce Behavior

Skills are merged across scopes with **first-wins by name (lowercase)**. Scope priority: workspace > assistant > global. A workspace skill named `my-tool` silently overrides an assistant or global skill with the same name.

Use this intentionally:

- Deploy a workspace-scoped override to temporarily patch a global skill for one session.
- Deploy an assistant-scoped copy (by copying the global skill directory files to the assistant skills directory) to customize a global skill for one assistant without affecting others.

## Common Mistakes

- **Wrong path separator**: always use forward slashes in paths passed to workspace tools, even on Windows.
- **Missing SKILL.md**: the scanner only recognises a skill directory if it contains `SKILL.md` at the top level.
- **Incorrect frontmatter**: `name` and `description` are both required. A missing field causes the skill to be silently skipped.
- **Skills directory not refreshed**: after writing files, the skills context is re-read at the start of the next agent turn. Inform the user that the new skill will be active from the next message onward.
