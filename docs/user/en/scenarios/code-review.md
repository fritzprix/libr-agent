---
title: Scenario - Automated Pull Request Code Review
---

# Scenario: Automated Pull Request Code Review

When you provide a Pull Request (PR) link, the agent reviews code changes automatically and provides structured feedback. The agent audits code quality, security vulnerabilities, naming conventions, and logic defects before merge.

---

## 🎯 Objectives

- Get instant feedback on pull requests without waiting for manual peer review.
- Detect bugs, security vulnerabilities, and style regressions early.
- Generate structured review summaries ready to post to your team.

---

## 📋 Step-by-Step Guide

### Step 1: Prepare the PR Link

Copy the URL of your Pull Request from GitHub, GitLab, or Bitbucket.

```
Example: https://github.com/your-org/your-repo/pull/42
```

### Step 2: Request Review from the Agent

Send the PR link to the agent with your review criteria.

**Basic review prompt:**

```
Review the pull request at https://github.com/your-org/your-repo/pull/42.
Summarize key modifications and identify potential issues.
```

**Comprehensive review prompt:**

```
Review https://github.com/your-org/your-repo/pull/42 across these five criteria:
1. Logic defects and edge-case bugs.
2. Security vulnerabilities and unsanitized inputs.
3. Code style, clarity, and readability.
4. Performance bottlenecks or redundant operations.
5. Unit and integration test coverage.
```

**Target specific frameworks:**

```
Review https://github.com/your-org/your-repo/pull/42.
Focus specifically on React state handling and TypeScript type soundness.
```

### Step 3: Review Structured Feedback

The agent analyzes the code diff and returns structured findings.

**Typical review output:**

```
📋 Pull Request Review Report

📊 Summary: 3 files changed, +120 -45 lines

✅ Passed:
- Code formatting complies with project standards.
- Existing regression tests pass.

⚠️ Warnings (2 items):
1. [Critical] Missing file extension validation in file upload handler.
   → Unchecked uploads allow malicious executable execution.
   → Fix: Add an explicit whitelist of allowed file extensions.

2. [Warning] Synchronous state update inside useEffect without dependencies.
   → Direct setState calls inside useEffect can trigger infinite re-renders.
   → Fix: Wrap state updates in useCallback or check change conditions.

💡 Suggestions:
- Wrap network requests in try-catch blocks for better error handling.
- Add JSDoc comments to public exports.
```

### Step 4: Apply Feedback and Re-verify

Update your code based on the agent's findings.

**Request automated code fixes:**

```
Propose code to fix warning #2. Wrap the setState logic inside useCallback.
```

**Request re-review after commit:**

```
I updated the branch. Re-review the pull request:
https://github.com/your-org/your-repo/pull/42
```

---

## 💡 Example Prompt Collection

| Purpose | Prompt |
| --- | --- |
| Basic review | `"Review the pull request code at https://github.com/.../pull/42."` |
| Focused review | `"Review this PR with focus only on security and performance."` |
| Diff summary | `"Provide a high-level summary of all architectural changes in this PR."` |
| Code patch | `"Draft a patch file to resolve finding #1 in this review."` |
| Comparative review | `"Compare architectural differences between PR #42 and PR #38."` |

---

## ⚙️ Configuration Tips

### Define a Specialized Review Persona

1. Open `/assistants` and select **New Assistant**.
2. System prompt:
   ```
   You are a Senior Software Engineer and Code Review Specialist.
   Inspect git diffs for logic bugs, security vulnerabilities, performance regressions, and style issues.
   Provide constructive, actionable suggestions with clear code examples.
   ```
3. Enable browser or git tools to inspect pull requests directly.

### Provide Project Checklist Rules

Provide repository-specific review standards to the agent:

```
When reviewing pull requests in this repository:
- Verify ESLint and Prettier compliance.
- Require co-located test files (*.test.ts).
- Enforce strict TypeScript types (no 'any').
- Verify defensive error handling for all asynchronous API calls.
```

---

## 🔗 Related Documentation

- [Assistants Guide](../guides/assistants.md) — Configure specialized code review personas
- [Playbooks Guide](../guides/playbooks.md) — Turn code review workflows into repeatable playbooks

---

## 🚀 Next Steps

| Project | Description | Difficulty |
| --- | --- | --- |
| Review Assistant | Configure a dedicated code review persona | ⭐⭐ |
| Review Playbook | Save your team's code review checklist as a playbook | ⭐⭐⭐ |
| Automated Review Trigger | Run PR review workflows automatically on new commits | ⭐⭐⭐ |
| Review Standards Doc | Formalize repository-specific review guidelines | ⭐⭐ |
| PR Diff Analytics | Compare code churn across related pull requests | ⭐⭐ |
| Refactoring Plan | Request a refactoring plan after code review approval | ⭐⭐⭐ |
