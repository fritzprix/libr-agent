---
title: Scenario - Automated Code Review
---

# Scenario: Automated Code Review

Learn how to leverage LibrAgent for analyzing pull requests, detecting anti-patterns, reviewing diffs, and suggesting refactoring.

---

## 🎯 Objectives

- Perform automated quality checks on local code diffs or commit ranges.
- Detect security vulnerabilities, code smells, and performance bottlenecks.
- Generate structured code review summaries with concrete refactoring snippets.

---

## 📋 Step-by-Step Guide

### Step 1: Specify Target Code & Scope

Prompt the agent with your target files, branches, or PR diffs:

```
Review the uncommitted changes in the src/components/ directory and highlight potential memory leaks or missing React dependency arrays.
```

### Step 2: Set Review Criteria

You can instruct the agent on specific coding conventions or guidelines:

```
Analyze the changes against our clean code criteria:
- Single Responsibility Principle (SRP)
- Proper TypeScript type safety (no explicit `any`)
- Error handling completeness
```

### Step 3: Inspect Review Report

The agent will execute tools (e.g. `workspace__readFile`, `workspace__runShell`) to review the code and output a structured review report:

```markdown
## 🔍 Code Review Summary

### ⚠️ Recommendations

1. **`useSettingsPageController.ts`**: Missing cleanup function in `useEffect`.
2. **`AIModelsTab.tsx`**: Implicit `any` type found on model configuration callback.

### 💡 Refactoring Snippet

...
```
