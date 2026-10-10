# Design Principles

## Concise is Key

The context window is a public good. Skills share it with the system prompt, conversation history, other skills' metadata, and the user request.

**Default assumption: the agent is already very smart.** Only add context it does not already have. Challenge each paragraph: "Does this justify its token cost?"

## Degrees of Freedom

Match specificity to task fragility:

- **High freedom** (text instructions): multiple valid approaches, context-dependent decisions
- **Medium freedom** (pseudocode/scripts with parameters): preferred pattern exists, some variation OK
- **Low freedom** (specific scripts, few parameters): fragile operations, consistency critical

## Progressive Disclosure

Three-level loading:

1. **Metadata** (name + description) — always in context
2. **SKILL.md body** — when skill triggers
3. **Bundled resources** — as needed (scripts may run without loading into context)

Keep SKILL.md under ~150 lines. Split detailed content into `references/`. Link references one level deep from SKILL.md.

### Patterns

- **High-level guide + references**: core workflow in SKILL.md, variants in separate files
- **Domain-specific organization**: `references/finance.md`, `references/sales.md`, etc.
- **Conditional details**: basic steps in SKILL.md, advanced in linked files

Avoid deeply nested references. For files >100 lines, add a table of contents at the top.

## Cross-Platform Compatibility

LibrAgent runs across Windows, macOS, and Linux. Skills and their instructions must never leave agents stranded due to OS-specific assumptions.

### 1. Dual Shell Command Recipes
- Never provide Bash-only command snippets when guiding an agent to run terminal commands.
- For commands with line continuations (`\`), environment variables (`$VAR`), or chaining (`&&`), provide both **Bash** and **PowerShell** invocations, or use single-line OS-neutral invocations.
  - Bash: `tool -p "$TASK" --model model-name`
  - PowerShell: `& tool -p "$TASK" --model model-name`
- Note: PowerShell 5.1 (default on Windows) does NOT support `&&`. Use `;` or separate sequential commands.

### 2. Safe Argument Passing & Quoting
- Multiline prompts or strings with spaces/quotes break easily when passed to CLI arguments.
- In PowerShell, unquoted variables like `-p $prompt` cause argument splitting (every word becomes a separate CLI positional arg). Always quote (`-p "$prompt"`) or use Here-Strings:
  ```powershell
  $TASK = @'
  Multi-line content
  '@
  & command -p "$TASK"
  ```
- When feasible, prefer passing file paths (`--file path/to/prompt.md`) over raw string interpolation to eliminate shell escaping hazards.

### 3. Bundled Scripts Parity
- When bundling automation scripts under `scripts/`:
  - Prefer cross-platform runtimes (Node.js or Python scripts) rather than shell-specific scripts (`.sh`).
  - If a shell script is provided (e.g. `run.sh`), provide the Windows counterpart (`run.ps1` or `run.bat`).
  - Never hardcode Unix-only paths like `/bin/bash` or `~/.local/bin` without platform detection or fallbacks.

### 4. File Paths and Encodings
- Use forward slashes (`/`) or platform-agnostic path joins in scripts and references.
- Ensure text files (especially handoffs or script outputs) use UTF-8 without BOM to prevent encoding mismatches on Windows (`CP949` / UTF-16LE).

## What NOT to Include

Do not add README.md, CHANGELOG.md, INSTALLATION_GUIDE.md, or other auxiliary docs. Only include files that help the agent do the job.
