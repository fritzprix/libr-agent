# Decomposition Rules

Properly dividing a task is critical for the success of the Divide-Conquer pattern.

## 📌 Decomposition Principles

1. **Independence**
   - Subtasks must not depend on the output of other subtasks. They must run in parallel without ordering dependencies.
   
2. **Clear Boundaries**
   - Each subtask must target a distinct, non-overlapping directory, file, or functional area to avoid git conflicts and logical overlaps.
   
3. **Optimal Sizing**
   - Micro-decomposition increases overhead (spawning too many sessions).
   - Macro-decomposition creates a single-session bottleneck.
   - **Recommendation:** Aim for **3 to 8** subtasks, respecting `maxConcurrentActiveSessions` (default: 4).

---

## 🚫 Anti-Patterns (Rework Required)

Redo your decomposition if any of the following apply:

### 1. Inter-dependent Subtasks
* **Bad:** 
  - Subtask A: "Design API specification."
  - Subtask B: "Implement frontend using Subtask A's API specs."
* **Fix:** Execute sequentially (use pipeline) or merge into a single subtask.

### 2. File Modification Conflict (Write-Write)
* **Bad:**
  - Subtask A: "Refactor format function in `src/utils.ts`."
  - Subtask B: "Add parse function to `src/utils.ts`."
* **Fix:** Isolate tasks by file or run them sequentially.

### 3. Vague Boundaries
* **Bad:**
  - Subtask A: "Refactor the codebase."
  - Subtask B: "Improve code comments."
* **Fix:** Define concrete directory or module boundaries (e.g., `src/features/auth` vs `src/features/billing`).
