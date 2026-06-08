---
name: agent-init
description: Analyze a workspace (code project, docs collection, or mixed) and generate a tailored agents.md (or AGENTS.md/CLAUDE.md/GEMINI.md) to onboard AI agents. Use when a user wants to initialize agent guidelines for a project, create agents.md, or refresh a generic template with project-specific content. Triggers: "agent init", "agents.md 생성", "워크스페이스 분석", "agent guideline 생성", "agent-init".
---

# Agent Init

Analyze any workspace and generate a tailored `agents.md` (or equivalent) based on its type.

> **Not this skill**
>
> | Skill                 | Use for                                               |
> | --------------------- | ----------------------------------------------------- |
> | **workspace-indexer** | Binary → Markdown conversion, `index.md` keyword maps |
> | **repo-wiki**         | `catalog.json`, `[[slug]]` links, backlinks           |
> | **soul-awakening**    | `SOUL.md` persona files                               |

## Workspace Types

Classify the workspace by exploring the root directory. Use approximate ratios — exact counts are not required.

| Type          | Criteria                                                                                                                                                                        |
| ------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **docs-site** | Docs framework config present (`docusaurus.config.*`, `vitepress`, `mkdocs.yml`, `nextra`, Astro Starlight) — treat as **docs** even if `package.json` exists                   |
| **docs**      | Markdown files ≥ 60% of meaningful content AND (`docs/` tree OR wiki-style `[[links]]` OR frontmatter in sample files) AND no dominant application code                         |
| **code**      | Build system exists (`package.json`, `Cargo.toml`, `pyproject.toml`, `go.mod`, `Makefile`, etc.) AND code extensions (`.ts`, `.rs`, `.py`, `.go`, `.java`, etc.) ≥ 40% of files |
| **mixed**     | Both code and docs each ≥ 30% — neither clearly dominates                                                                                                                       |
| **unknown**   | None of the above — use generic template                                                                                                                                        |

**Tie-breakers**

- `package.json` only for lint/format tooling + mostly `.md` → **docs**
- Infra/config-only (Terraform, K8s YAML, Helm) with no app code → **unknown** (fill `generic.md` with infra notes)
- Monorepo: classify from root; note major packages in `{file_structure}`

## Workflow

### 1. Classify the workspace

Explore the workspace root (not the skill Base Directory). Check:

- File extensions (code vs MD ratio)
- Build system and docs-framework config files
- MD frontmatter in 3–5 sample files (optional — not required for docs)
- Directory patterns (`src/`, `docs/`, `lib/`, etc.)

**Code discovery checklist**

- Read `README.md`, `package.json` / `Cargo.toml` / `pyproject.toml`
- List scripts or Makefile targets
- Detect linter/format config (`.eslintrc`, `rustfmt`, `clippy`, `ruff`, `prettier`)
- Tree depth 2: `src/`, `lib/`, `tests/`, `docs/`

**Docs discovery checklist**

- Sample 3–5 MD files for frontmatter and link patterns
- Check `_meta/`, `.obsidian/`, `mkdocs.yml`, `docusaurus.config.*`, `docs/`
- Note audience and generated vs hand-written docs

### 2. Analyze the workspace

Based on type, gather information:

**Code workspace**

- Language, framework, package manager, build system
- Test framework, linting/formatting tools
- Component patterns, service layer, error handling style
- Directory structure (top 2 levels)
- Validation command, CI workflow, logging conventions, type-safety rules

**Docs workspace**

- File tree (top 2 levels)
- Frontmatter patterns if present; otherwise note "none"
- Linking style (relative, wiki-style, markdown)
- Directory conventions, workflow notes, audience

**Mixed workspace**

- Both code and docs sections above

### 3. Select template

Read from `references/templates/` (relative to this skill's Base Directory):

| Type                  | Template             |
| --------------------- | -------------------- |
| `code`                | `code-workspace.md`  |
| `docs` or `docs-site` | `docs-workspace.md`  |
| `mixed`               | `mixed-workspace.md` |
| `unknown`             | `generic.md`         |

Fill all placeholders with discovered info. **Never leave `{...}` tokens in the output.**

**Placeholder rules (in order)**

1. **Fill from evidence** — cite real files, scripts, or config paths
2. **Use a fallback sentence** — when the section adds value but nothing was detected (see table below)
3. **Remove the section** — when the entire `##` / `###` block has no evidence and no useful generic fallback

**Hard-to-detect placeholders — where to look and what to write**

| Placeholder                   | Look for                                                                                             | If not found                                                                                                                                   |
| ----------------------------- | ---------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| `{ci_workflow}`               | `.github/workflows/`, `.gitlab-ci.yml`, `Jenkinsfile`, `azure-pipelines.yml`, CI badges in README    | `No CI configuration detected. Run tests locally before opening PRs.` — or remove **CI / Pull Requests** section                               |
| `{security_notes}`            | `SECURITY.md`, `docs/security*`, Dependabot/Renovate config, secret-scanning config                  | `No project-specific security policy detected. Do not commit secrets; report vulnerabilities to maintainers.` — or remove **Security** section |
| `{changelog_policy}`          | `CHANGELOG.md`, release docs, conventional-commits mention                                           | `No changelog policy detected. Document notable changes in PR descriptions.` — or remove changelog bullet                                      |
| `{validation_command}`        | `package.json` scripts (`lint`, `test`, `check`, `validate`), `Makefile`, `justfile`, project README | Primary lint/test command from scripts, or remove **Validation** line                                                                          |
| `{env_setup}`                 | README setup, `.nvmrc`, `.node-version`, `rust-toolchain.toml`, `pyproject.toml` python version      | `See README.md for environment setup.` — only if README has setup steps; otherwise remove section                                              |
| `{key_architecture_patterns}` | README architecture section, `docs/architecture*`, monorepo layout                                   | Summarize from directory layout + README; if neither exists, remove section                                                                    |
| `{type_safety_rules}`         | `tsconfig.json` strict flags, `mypy.ini`, Clippy lints, ESLint type rules                            | `Follow language defaults; no extra type-safety tooling detected.` — or remove section                                                         |
| `{error_handling}`            | README, CONTRIBUTING, representative source patterns                                                 | Describe only if a consistent pattern is visible in code or docs; otherwise remove section                                                     |
| `{logging_conventions}`       | Centralized logger module, logging config, README mention                                            | `No centralized logging convention detected.` — or remove **Logging** section                                                                  |
| `{performance_notes}`         | Perf docs, benchmarking scripts, README performance section                                          | Remove **Performance** section                                                                                                                 |
| `{review_process}`            | `CONTRIBUTING.md`, `CODEOWNERS`, PR template                                                         | `No formal review process documented. Use PRs for all changes.` — or remove review bullet                                                      |
| `{update_policy}`             | CONTRIBUTING, maintenance docs                                                                       | `Update docs when changing user-facing behavior.` — or remove update bullet                                                                    |
| `{writing_guidelines}`        | CONTRIBUTING, style guide in `docs/`                                                                 | `Match existing document tone and structure.` — or remove writing bullet                                                                       |
| `{audience}`                  | README intro, docs site config                                                                       | `Developers and contributors working in this repository.`                                                                                      |
| `{frontmatter_schema}`        | Sample MD frontmatter fields                                                                         | `none` (literal text)                                                                                                                          |
| `{asset_conventions}`         | `docs/images/`, `static/`, `assets/` in docs tree                                                    | `Place assets alongside related documents unless a project convention exists.`                                                                 |
| `{generated_vs_handwritten}`  | Autogen markers, `<!-- generated -->`, build output paths                                            | `Hand-written unless a path or header indicates generated content.`                                                                            |
| `{quality_checks}`            | CONTRIBUTING, CI doc checks, markdown lint config                                                    | `Spell-check and verify links before merging.` — or remove section                                                                             |
| `{additional_notes}`          | Anything else noteworthy                                                                             | Remove **Notes** section if empty                                                                                                              |

Do not invent tools, commands, or policies. Prefer removing a section over guessing.

### 4. Generate the file

**Output location:** workspace root — not the skill Base Directory.

**Target filename** (first match wins):

1. Existing guideline file in root (`agents.md`, `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`)
2. `agents.md` if none exists

**If the file already exists:** follow `references/merge-policy.md`.

## Quality Checks

Before finishing:

1. No unfilled placeholders (`{...}`) remain
2. Project name and tech stack match actual files (not guessed)
3. Directory structure matches layout at depth 2
4. Coding conventions cite real config files (`eslint.config.*`, `rustfmt.toml`, etc.)
5. File is well-formatted Markdown
