---
name: repo-wiki
description: >
  Unified repository wiki system: bi-directional linking, metadata-driven search, binary
  document conversion, and full-file indexing. Manages catalog.json index, frontmatter
  metadata injection, slug-based link resolution, backlink generation, structured directory
  routing (core/reference/drafts/archive), AND binary-to-text conversion (PDF/DOCX/PPTX/XLSX)
  with workspace index.md generation. Use when: (1) organizing docs/ into a wiki with
  bi-directional links, (2) generating catalog.json, (3) injecting frontmatter, (4) converting
  relative links to [[slug]] format, (5) building backlinks, (6) validating broken links,
  (7) migrating flat docs into core/reference/drafts/archive, (8) converting binary documents
  (PDF/DOCX/PPTX/XLSX) to Markdown, (9) building workspace index.md with file inventory and
  keyword map. Triggers: "repo wiki", "catalog.json", "bi-directional links", "backlinks",
  "docs-tool", "문서 위키", "이진 파일 변환", "pdf 변환", "index.md 생성", "repo-wiki".
---

# Repo Wiki

Unified repository wiki system: bi-directional linking + binary conversion + full indexing.

## Quick Start

```bash
# Run the python commands relative to the skill's Base Directory
# Wiki initialization (metadata, links, backlinks)
python3 scripts/repo-wiki.py wiki init
python3 scripts/repo-wiki.py wiki add --all
python3 scripts/repo-wiki.py wiki backlinks --build
python3 scripts/repo-wiki.py wiki links --check

# Binary conversion + indexing
python3 scripts/repo-wiki.py convert --root .
python3 scripts/repo-wiki.py index --root .
```

## Commands

### Wiki Commands (metadata, links, backlinks)

#### `wiki init`

Generate catalog.json, _index.md, and configuration.

```bash
python3 repo-wiki.py wiki init
```

Creates:
- `_meta/catalog.json` — Full document index with metadata
- `_meta/backlinks/` — Directory for reverse-link data
- `_index.md` — Curated landing page
- `.repo-wikirc` — Configuration file

#### `wiki add [--all] [--file PATH]`

Analyze MD file, extract slug, inject frontmatter, update catalog.

```bash
python3 repo-wiki.py wiki add --file guides/getting-started.md
python3 repo-wiki.py wiki add --all
```

Extracts slug from filename (kebab-case). Reads/writes frontmatter. Adds missing fields:
`title`, `slug`, `status`, `category`, `tags`, `created`, `updated`.

#### `wiki link <from-file> <to-slug> [--section HEADING]`

Convert a link to slug format.

```bash
python3 repo-wiki.py wiki link guides/getting-started.md agent-workflow-architecture
python3 repo-wiki.py wiki link guides/getting-started.md mcp-channel-support --section transport
```

Replaces relative paths with `[[slug]]` or `[[slug#section]]`.

#### `wiki backlinks --build [--file PATH]`

Generate backlink data for all documents or a single file.

```bash
python3 repo-wiki.py wiki backlinks --build
python3 repo-wiki.py wiki backlinks --file core/architecture/agent-workflow-architecture.md
```

Scans all MD files for `[[slug]]` references. Writes to `_meta/backlinks/<slug>.json`.
Appends "Referenced by" section to the document.

#### `wiki links --check [--fail-on-broken]`

Validate all `[[slug]]` links against catalog.json.

```bash
python3 repo-wiki.py wiki links --check
python3 repo-wiki.py wiki links --check --fail-on-broken
```

Exit code 0 if all links valid, 1 if broken. For CI integration.

#### `wiki migrate --to STRUCTURED`

Restructure flat docs into core/reference/drafts/archive.

```bash
python3 repo-wiki.py wiki migrate --to structured
```

Moves files based on frontmatter `status`:
- `stable` → `core/`
- `experimental` → `drafts/`
- `deprecated` → `archive/`
- `draft` → `drafts/`

#### `wiki status`

Show document status summary.

```bash
python3 repo-wiki.py wiki status
```

Prints counts by status and category.

### Convert Commands (binary → text)

#### `convert [--root PATH] [--formats EXTENSIONS] [--out DIR] [--overwrite]`

Convert binary documents to Markdown.

```bash
python3 repo-wiki.py convert --root .
python3 repo-wiki.py convert --root . --formats pdf docx --overwrite
python3 repo-wiki.py convert --root . --out ./converted
```

| Format | Library | Output |
|--------|---------|--------|
| PDF | pymupdf | `## Page N` sections |
| DOCX | python-docx | Headings → `#`/`##`, tables → Markdown |
| PPTX | python-pptx | `## Slide N` sections |
| XLSX | openpyxl | `## Sheet: <name>` + Markdown tables |

### Index Commands (file inventory + keyword map)

#### `index [--root PATH] [--out PATH] [--dry-run] [--ignore-dirs DIRS]`

Build `index.md` with file inventory, binary document list, and keyword map.

```bash
python3 repo-wiki.py index --root .
python3 repo-wiki.py index --root . --out docs/index.md
python3 repo-wiki.py index --root . --ignore-dirs node_modules .git
```

`index.md` structure:
- **File inventory** — Directory tree with sizes
- **Binary document list** — Converted status (links to `.md` when available)
- **Keyword map** — Auto-extracted from `#`/`##`/`###` headings

### Unified Runner

#### `run [--root PATH] [--skip-convert] [--skip-index] [--dry-run]`

Execute conversion + indexing + wiki in sequence.

```bash
python3 repo-wiki.py run --root .
python3 repo-wiki.py run --root . --skip-convert   # wiki only
python3 repo-wiki.py run --root . --skip-index     # convert only
```

## Linking Patterns

Use `references/linking-patterns.md` for `[[slug]]` syntax, cross-file linking, section references.

## Metadata Schema

Use `references/metadata-schema.md` for frontmatter field definitions and validation rules.

## Prerequisites

```bash
# Automatic install (run from the skill's Base Directory)
python3 scripts/install_deps.py

# Manual install
pip install pymupdf python-docx python-pptx openpyxl
```

## Workflow

### Phase 1: Wiki (metadata + links)

1. `repo-wiki.py wiki init` — Generate catalog.json, _index.md
2. `repo-wiki.py wiki add --all` — Inject frontmatter into all files
3. `repo-wiki.py wiki link` — Convert relative links to `[[slug]]` (manual or bulk)
4. `repo-wiki.py wiki backlinks --build` — Generate reverse references
5. `repo-wiki.py wiki links --check` — Validate before committing
6. `repo-wiki.py wiki migrate --to structured` — Reorganize into core/reference/drafts/archive

### Phase 2: Conversion (binary → text)

1. `repo-wiki.py convert --root .` — Convert PDF/DOCX/PPTX/XLSX to Markdown
2. `repo-wiki.py convert --root . --formats pdf docx` — Selective conversion
3. `repo-wiki.py convert --root . --overwrite` — Reconvert existing files

### Phase 3: Indexing (full inventory)

1. `repo-wiki.py index --root .` — Build index.md with file inventory + keyword map
2. `repo-wiki.py index --root . --dry-run` — Preview before writing

### Phase 4: Full Workflow

1. `repo-wiki.py run --root .` — Execute all phases in sequence
2. `repo-wiki.py run --root . --skip-convert` — Wiki only (MD files)
3. `repo-wiki.py run --root . --skip-index` — Wiki + convert only

## Notes

- `.git`, `__pycache__`, `node_modules`, `.github`, `venv` ignored by default
- Existing converted files not regenerated unless `--overwrite`
- Scanned PDFs (image-only) need separate OCR step
- For large workspaces, use `--max-keyword-files` to cap keyword map size
