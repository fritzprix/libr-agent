# Gate Criteria & Limit Guidelines

Define clear audit rules and safety limits to optimize the Creator-Reviewer workflow.

## 📌 Defining Quality Criteria

Reviewers must audit against **objective, measurable metrics** rather than subjective feelings.

1. **Code Gate**
   - Must compile without errors.
   - 100% test pass rate.
   - Zero TypeScript/Rust compiler warnings or lint errors.
   - No `any` type casting.

2. **Content/Translation Gate**
   - 100% compliance with glossary mappings.
   - Markdown syntax validity (no broken headings).
   - No untranslated lines.

---

## 🚫 Loop Limits & Cost Safeguards

To prevent excessive token consumption and infinite loops:

* **Max Rework Limit:**
  - Cap rework cycles at **3** loops.
  - If a gate fails 3 times, abort and prompt the user or parent session for manual intervention.
  
* **Structured Feedback Format:**
  - Instruct reviewers to output audit failures strictly under these headers:
    - **`[FAILED_CRITERIA]`**: Checklist of failed rules.
    - **`[REQUIRED_ACTION]`**: Precise files and edits required.
    - **`[SUGGESTION]`**: Optional cleanups.

---

## 📋 Prompt Template Example

### Gatekeeper (Reviewer) Prompt:
```markdown
You are a PR auditor and QA gatekeeper.
Verify the file `/shared/workspace/src/math.ts` against the following rules:
1. No 'any' type castings.
2. Floating point overflow guards must be implemented.

If all criteria are met, print [APPROVED]. Otherwise, print [REJECTED] along with the structured audit failures.
```
