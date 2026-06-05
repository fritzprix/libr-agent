# Repo Wiki Linking Patterns

This document defines how document linking patterns are parsed and resolved inside the repo-wiki system.

---

## 1. Slug-based Linking Format
Instead of relying on fragile relative paths, files are linked via their unique document slug:
- **Format**: `[[target-slug]]`
- **Example**: To link to `guides/getting-started.md` (which has `slug: getting-started`), use `[[getting-started]]`.

## 2. Heading/Section Linking
You can link to specific headings inside a target document:
- **Format**: `[[target-slug#heading-title-kebab-case]]`
- **Example**: `[[getting-started#prerequisites]]`

## 3. Backlink Injection Rules
The `backlinks` tool scans all Markdown files in the workspace for occurrences of `[[slug]]`.
- It maps these reverse references and populates the `_meta/backlinks/<slug>.json` index.
- A "Referenced by" section is automatically appended to the bottom of target files during building phase.
