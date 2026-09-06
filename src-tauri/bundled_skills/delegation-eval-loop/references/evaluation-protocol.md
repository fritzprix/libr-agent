# Evaluation Protocol & Layered Verification

This reference details the exact checks and failure actions for each layer in the evaluation stack.

## Layer 1: Deterministic Verification

Deterministic checks require zero LLM interpretation and produce unambiguous binary outcomes (exit code 0 or non-zero).

### Commands to Run by Project Type

- **LibrAgent Full Validation**:
  ```bash
  pnpm refactor:validate
  ```
- **Rust Backend**:
  ```bash
  cargo check --all-targets
  cargo test <test_filter>
  cargo clippy -- -D warnings
  ```
- **Frontend (TypeScript / React)**:
  ```bash
  pnpm tsc --noEmit
  pnpm lint
  pnpm test
  ```

### Rules
- Never accept a sub-agent's claim that tests passed without either seeing the exact command output in its trajectory or running the command yourself.
- If Layer 1 fails, do not proceed to Layer 2, 3, or 4. Terminate the evaluation immediately and return the compiler/test error output.

---

## Layer 2: Invariant Gate

Invariant gates protect against security, policy, and boundary violations. A task that passes all unit tests but violates an invariant is an automatic hard failure.

### Key Invariants to Check

1. **Workspace Protection**:
   - For LibrAgent: check if protected paths (`src/`, `src-tauri/`, `docs/`, `.github/`, `README*.md`) were modified when the sub-agent was only authorized to work in `.libragent/work/` or `coordination/`.
2. **File Scope**:
   - Run `git status --porcelain` to verify only the agreed files were added or modified.
3. **Secret & Credential Leakage**:
   - Ensure no API keys, tokens, or local absolute paths were hardcoded into committed files.
4. **Clean Workspace**:
   - Ensure no leftover scratch files (e.g. `test.tmp`, `.scratch.py`) remain untracked in the working directory.

### Failure Handling
- If an invariant is violated, command the sub-agent to revert the forbidden modifications immediately before doing any further work.

---

## Layer 3: Trajectory & Reliability Audit

Inspect the sub-agent's execution trace and self-reported response to detect unrecovered errors, superficial completions, or unreliable "success".

### What to Look For
1. **Mocking / Placeholders**: Did the sub-agent put `// TODO: implement later` or placeholder return values to make the compiler pass?
2. **Error Suppression**: Did the sub-agent wrap a failing call in `try { ... } catch {}` or `let _ = ...` to artificially silence an error?
3. **Looping Fatigue**: Did the sub-agent exhaust its turn limit and give up while claiming partial success?
4. **Claim vs Fact**: Does the text say "I updated the database schema" but `git diff` shows the migration file was never touched?
5. **Reliability hard-fail** (treat as Layer 3 fail even if Layer 1 was never run):
   - session `error` / cancelled / timed out before a real deliverable
   - empty or truncated final text with "done" language
   - circuit-break / tool-loop hard stop without completing acceptance criteria
   - child asks the parent to trust self-grading instead of providing command output

Parent acceptance requires parent-visible evidence. Child self-score is never Layer 1.

---

## Layer 4: Semantic Review (Parent LLM)

Once Layers 1-3 pass, the parent agent evaluates the qualitative aspects:
1. **Original Intent**: Does the code change directly solve what the user asked for, or did the sub-agent solve an easier, adjacent problem?
2. **Side Effects**: Are there any regressions or architectural antipatterns introduced?
3. **Clarity**: Is the documentation, naming, and error handling up to project standards?

Do not use Layer 4 to "curve" a Layer 2/3 failure into a pass.

---

## Escalation Protocol

If the child session fails verification after **3 to 4 attempts**:
1. Do not enter an infinite retry loop.
2. Stop delegating to that child session.
3. Summarize the blocker clearly for the user:
   - What the sub-agent was tasked with.
   - What the sub-agent attempted.
   - The exact persistent failure (e.g. compiler error, invariant violation, reliability).
   - Recommended next steps for the user or manual intervention.
