# Git Diff & Codebase Change Analysis Guide

This guide details how to analyze recent Git commits and diffs, categorize changes, and map codebase modifications to user-facing documentation in `docs/user/` and VitePress.

---

## 1. Extracting Git Changes

### A. List Recent Commit Summaries

```bash
git log --oneline -n 20
```

### B. List Changed Files by Category

```bash
git diff --name-status HEAD~10 HEAD
```

### C. Run Automated Gap Detection Script

```bash
python .agents/skills/vitepress-doc-sync/scripts/detect_doc_gaps.py --since HEAD~10
```

---

## 2. Mapping Code Changes to Documentation Files

| Changed Codebase Area          | Example File Paths                                          | Target Documentation Pages (`docs/user/`)                                 |
| :----------------------------- | :---------------------------------------------------------- | :------------------------------------------------------------------------ |
| **Builtin / Custom MCP Tools** | `src-tauri/src/mcp/builtin/*`                               | `guides/builtin-tools.md`, `guides/extensions.md`, `guides/custom-mcp.md` |
| **Skills System & Scopes**     | `.agents/skills/*`, `src-tauri/src/agent/skills/*`          | `guides/skills.md`                                                        |
| **Sub-agents & Delegation**    | `src-tauri/src/agent/lifecycle/*`, `src/context/`           | `guides/sub-agents.md`                                                    |
| **Assistants & Models**        | `src-tauri/src/agent/config*`, `src/components/assistant/`  | `getting-started/connecting-models.md`, `guides/assistants.md`            |
| **Playbooks & Automation**     | `src-tauri/src/mcp/builtin/playbook/*`                      | `guides/playbooks.md`, `guides/automation.md`                             |
| **CLI & Setup**                | `scripts/manage-logs.js`, `package.json`                    | `getting-started/5-minute-tutorial.md`, `faq/common-questions.md`         |
| **Session Management**         | `src-tauri/src/commands/agent_commands/session_commands.rs` | `guides/sessions.md`                                                      |

---

## 3. Auditing Principles

1. **New Feature Check**: If a new feature or command was added in Rust/TypeScript, verify if a corresponding section exists in both Korean (`docs/user/*.md`) and English (`docs/user/en/*.md`).
2. **Breaking / Interface Changes**: If an existing CLI argument, tool name, or configuration property changed, search for outdated parameter names across all `.md` files in `docs/user/`.
3. **Sidebar Alignment**: Check if newly added `.md` files are registered in `website/.vitepress/config.ts` (`koSidebar` and `enSidebar`).
