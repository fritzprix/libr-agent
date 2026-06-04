# Repo Wiki Metadata Schema

This schema defines the frontmatter requirements for all Markdown files indexed in the wiki catalog.

---

## Required Frontmatter Fields

All documents should declare the following frontmatter fields at the top:

```yaml
---
title: "Document Title"
slug: "document-slug-in-kebab-case"
status: "draft | stable | experimental | deprecated"
category: "guides | core | reference | archive"
tags: [tag1, tag2]
created: 2026-06-04T12:00:00Z
updated: 2026-06-04T14:30:00Z
---
```

## Field Validation Rules

1. **`slug`**:
   - Must be uniquely defined across the catalog.
   - Format: Only lowercase letters, numbers, and hyphens (`[a-z0-9-]`).
2. **`status`**:
   - Determines directory migration routing (e.g. `stable` moves to `core/`, `deprecated` moves to `archive/`).
3. **`created` / `updated`**:
   - ISO-8601 formatted timestamp strings.
