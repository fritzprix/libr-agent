# VitePress Formatting & Configuration Guide

This guide covers VitePress formatting standards, multi-language structure, sidebar navigation updates, and build validation rules for LibrAgent documentation.

---

## 1. Directory Structure

- **VitePress Config**: `website/.vitepress/config.ts`
- **Korean Docs (Default Root)**: `docs/user/`
- **English Docs**: `docs/user/en/`

VitePress uses `srcDir: '../docs/user'`, mapping `/getting-started/5-minute-tutorial` to `docs/user/getting-started/5-minute-tutorial.md`.

---

## 2. Frontmatter & Markdown Formatting

### Page Frontmatter

Every user doc should include frontmatter metadata when appropriate:

```yaml
---
title: 스킬 (scope·배포)
description: LibrAgent 스킬 종류, 스코프 우선순위 및 배포 가이드
---
```

### Custom Containers

Use VitePress container syntax for callouts:

- `::: tip` (Tips & Best Practices)
- `::: info` (Informational Notes)
- `::: warning` (Important Warnings / Cautions)
- `::: danger` (Critical Safety Alerts)

Example:

```markdown
::: tip
`pnpm refactor:validate`를 실행하면 모든 빌드 및 린트 검증을 한 번에 수행할 수 있습니다.
:::
```

### Code Blocks

Use explicit language tags for syntax highlighting (e.g. `bash`, `typescript`, `rust`, `json`).

---

## 3. Sidebar & Navigation Configuration

When creating a new markdown page or reorganizing existing pages, update `website/.vitepress/config.ts`:

1. Add entry to `koSidebar`:

```typescript
{ text: '새 기능 가이드', link: '/guides/new-feature' }
```

2. Add corresponding entry to `enSidebar`:

```typescript
{ text: 'New Feature Guide', link: '/en/guides/new-feature' }
```

---

## 4. Build Validation & Dead Link Checks

Always run VitePress build to ensure there are no dead links or syntax errors before committing changes:

```bash
pnpm --filter website build
```

or run full validation:

```bash
pnpm refactor:validate
```
