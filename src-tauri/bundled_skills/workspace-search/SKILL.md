---
name: workspace-search
description: |
  Generate a hybrid (BM25 + Semantic) search index for the workspace, perform efficient incremental updates,
  and run fast queries combining keyword ranking with vector similarity.
  Use when the user asks to search the workspace, build/update a search index, perform semantic search,
  find files or code segments by keyword or meaning, or configure search embeddings.
---

# Workspace Search

This skill provides workspace indexing and hybrid search capabilities, combining classic lexical search (BM25) with semantic vector search (via OpenAI, Gemini, Ollama, or local SentenceTransformers).

## Path Conventions

All internal paths mentioned in this skill are relative to this skill's root directory:
`C:/Users/SKTelecom/my_works/libr-agent/src-tauri/bundled_skills/workspace-search`

- In script commands below, replace `<skill-dir>` with the absolute path of this skill.

## Script List

| Script | Role |
| --- | --- |
| [indexer.py](file:///C:/Users/SKTelecom/my_works/libr-agent/src-tauri/bundled_skills/workspace-search/scripts/indexer.py) | **Index Generator & Incremental Updater** — Creates or updates lexical and vector search indexes. |
| [search.py](file:///C:/Users/SKTelecom/my_works/libr-agent/src-tauri/bundled_skills/workspace-search/scripts/search.py) | **Hybrid Search Engine** — Executes queries using BM25, Semantic, or Hybrid (RRF) search. |
| [install_deps.py](file:///C:/Users/SKTelecom/my_works/libr-agent/src-tauri/bundled_skills/workspace-search/scripts/install_deps.py) | **Dependency Installer** — Verifies and installs packages needed for offline local embeddings. |
| [utils.py](file:///C:/Users/SKTelecom/my_works/libr-agent/src-tauri/bundled_skills/workspace-search/scripts/utils.py) | **Search Utilities** — Internal tokenization, chunking, and API embedding handlers. |

## Prerequisites

By default, lexical search (BM25) and cloud API embeddings (OpenAI, Gemini, Ollama) work with **zero external dependencies**.
To use local offline vector embeddings, install `sentence-transformers`:

```bash
# Check dependencies
python C:/Users/SKTelecom/my_works/libr-agent/src-tauri/bundled_skills/workspace-search/scripts/install_deps.py --check

# Install sentence-transformers for local embeddings
python C:/Users/SKTelecom/my_works/libr-agent/src-tauri/bundled_skills/workspace-search/scripts/install_deps.py
```

## Workflow

### Task A: Generate/Update the Search Index

Run [indexer.py](file:///C:/Users/SKTelecom/my_works/libr-agent/src-tauri/bundled_skills/workspace-search/scripts/indexer.py) to build the initial search index or incrementally update it. The index is stored inside `.libragent/search_index/` in the workspace root.

```bash
# Default indexing (auto-detects provider/keys in environment)
python C:/Users/SKTelecom/my_works/libr-agent/src-tauri/bundled_skills/workspace-search/scripts/indexer.py --root .

# Force a complete rebuild of the index (ignores existing cache)
python C:/Users/SKTelecom/my_works/libr-agent/src-tauri/bundled_skills/workspace-search/scripts/indexer.py --root . --rebuild

# Use OpenAI embeddings explicitly
python C:/Users/SKTelecom/my_works/libr-agent/src-tauri/bundled_skills/workspace-search/scripts/indexer.py --root . --provider openai --model text-embedding-3-small

# Use Gemini embeddings explicitly
python C:/Users/SKTelecom/my_works/libr-agent/src-tauri/bundled_skills/workspace-search/scripts/indexer.py --root . --provider gemini --model models/text-embedding-004

# Use local sentence-transformers offline (requires install_deps.py)
python C:/Users/SKTelecom/my_works/libr-agent/src-tauri/bundled_skills/workspace-search/scripts/indexer.py --root . --provider local --model all-MiniLM-L6-v2

# Build lexical-only search index (BM25, zero dependencies, no API calls)
python C:/Users/SKTelecom/my_works/libr-agent/src-tauri/bundled_skills/workspace-search/scripts/indexer.py --root . --provider none
```

#### How Incremental Updates Work
The indexer computes SHA256 hashes of all text source files. During subsequent runs:
1. Deleted files are removed from the index.
2. Unchanged files are skipped (retains cached chunks & embeddings).
3. Modified or new files are chunked and re-indexed.
4. Global statistics (Average Document Length, Document Frequencies) are recomputed.

This ensures indexing is extremely fast even on large codebases.

---

### Task B: Search the Workspace

Run [search.py](file:///C:/Users/SKTelecom/my_works/libr-agent/src-tauri/bundled_skills/workspace-search/scripts/search.py) to query the index.

```bash
# Hybrid Search (RRF - Reciprocal Rank Fusion of BM25 + Semantic)
python C:/Users/SKTelecom/my_works/libr-agent/src-tauri/bundled_skills/workspace-search/scripts/search.py --root . --query "AgentSessionManager think loop" --type hybrid

# Lexical Search Only (BM25)
python C:/Users/SKTelecom/my_works/libr-agent/src-tauri/bundled_skills/workspace-search/scripts/search.py --root . --query "struct ServiceContext" --type bm25

# Semantic Search Only
python C:/Users/SKTelecom/my_works/libr-agent/src-tauri/bundled_skills/workspace-search/scripts/search.py --root . --query "session isolated mcp servers" --type semantic

# Get Top-K results (default is 10)
python C:/Users/SKTelecom/my_works/libr-agent/src-tauri/bundled_skills/workspace-search/scripts/search.py --root . --query "safeInvoke wrapper" --top-k 5

# Format output as JSON (useful for programmatic parsing or scripts)
python C:/Users/SKTelecom/my_works/libr-agent/src-tauri/bundled_skills/workspace-search/scripts/search.py --root . --query "error logging" --json
```

## Search Fusion Details

- **BM25 Search**: Matches exact keywords using BM25 relevance scoring. Best for finding specific code symbols, class names, or API definitions.
- **Semantic Search**: Generates an embedding of the query and ranks document chunks based on cosine similarity. Best for concept-based questions, natural language queries, or finding related functionality where keywords differ.
- **Hybrid Search**: Combines lexical rank and semantic rank using **Reciprocal Rank Fusion (RRF)**:
  $$RRF\_Score(d) = \frac{1}{60 + rank_{BM25}(d)} + \frac{1}{60 + rank_{Semantic}(d)}$$
  This produces a robust list of results that capture both literal keyword matches and semantic meaning.

## Notes

- Checked-in index files inside the `.libragent/` directory are automatically ignored by Git/scans to prevent code bloat.
- When query embeddings are required, the search script automatically retrieves the provider and model configurations stored during indexing in `meta.json`.
- Result paths display as clickable Markdown links mapped directly to their line ranges.
