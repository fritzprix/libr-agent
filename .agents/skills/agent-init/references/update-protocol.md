# 🔄 On-the-Fly Guideline Self-Healing & Update Protocol

> **Mandatory Agent Rule**: AI agents must treat project guidelines as living documents.
> Whenever stale information, changed code structure, or new user instructions are detected during work, **immediately update** the relevant guideline files.

---

## 🔍 1. When to Trigger an Immediate Update

An agent MUST trigger an immediate update when any of the following conditions occur:

| Drift Event                            | Examples                                                                                   | Target File to Update                                                |
| -------------------------------------- | ------------------------------------------------------------------------------------------ | -------------------------------------------------------------------- |
| **Directory / File Structure Change**  | New core modules added, files renamed/moved, new feature folder created                    | `docs/guidelines/architecture-and-files.md`                          |
| **Command / Build Script Change**      | `package.json` scripts updated, new test runner, new validation pipeline command           | `docs/guidelines/coding-standards.md` & `agents.md` Cheat Sheet      |
| **New User Directives / Precautions**  | User says "Never use X library", "Always write comments in Korean", "Avoid editing file Y" | `docs/guidelines/persona-and-rules.md` & `agents.md` Directives      |
| **Persona / Tone Request**             | User specifies desired vibe (e.g., concise, informal, humorous, or strict)                 | `docs/guidelines/persona-and-rules.md` & `agents.md` Persona         |
| **New Coding / Architectural Pattern** | Introduction of a new service wrapper, error handling pattern, or state library            | `docs/guidelines/coding-standards.md` or `architecture-and-files.md` |

---

## 🛠️ 2. Step-by-Step Update Execution

1. **Perform Surgical Edits**:
   - Use `workspace__strReplace` to make targeted edits to the relevant modular guide file under `docs/guidelines/`.
   - Do not overwrite entire files unless a full rewrite is specifically requested.

2. **Sync `agents.md` Entrypoint**:
   - If the update affects high-level information (such as command cheat sheet, user directives summary, or persona summary in `agents.md`), update `agents.md` as well.
   - Keep `agents.md` concise. Detailed descriptions stay inside `docs/guidelines/*.md`.

3. **Log & Inform**:
   - Briefly inform the user in 1 sentence about the guideline update made during the task (e.g. _"Updated `docs/guidelines/architecture-and-files.md` with the new auth module structure."_).
