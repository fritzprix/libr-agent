---
argument-hint: '[optional: target file or area]'
description: Analyze code for performance issues and propose low-risk optimizations
---

# Performance optimization

Review the referenced code and suggest safe optimizations.

## Consider

- Avoid unnecessary re-renders (memoization, dependency arrays)
- Reduce bundle impact (lazy import, tree-shaking friendly patterns)
- Database/IndexedDB query efficiency
- Async boundaries on CPU-heavy work (WebWorker or Rust)
- Rust: avoid allocations, prefer slices/iterators, use async where appropriate

## Output

- Quick wins (bulleted)
- Before/after snippets when small and illustrative
- Risk notes and test suggestions
