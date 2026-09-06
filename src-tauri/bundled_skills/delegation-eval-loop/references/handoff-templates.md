# Delegation Handoff & Feedback Templates

Use these templates when briefing sub-agents or sending structured re-steering feedback via `agent__messageToSession`.

Mechanics (spawn, isolation, tool names): **`delegate`**. This file is for **generator–evaluator** contracts and rejects.

---

## 0. Sprint Contract Checklist (before spawn)

Fill these before `agent__startSession` / assignment message:

- [ ] Objective is one concrete deliverable (not "improve X")
- [ ] Authorized paths listed; forbidden paths listed
- [ ] At least one **Deterministic** acceptance checkbox (exact command)
- [ ] Invariant checkbox (`git status` / scope) when the child can write files
- [ ] Child told to put proof in **final text**, not scratchpad
- [ ] Parent will run or inspect Layer 1–3 (child does not self-grade acceptance)

If you cannot name a verifiable check, either keep the task on soft `delegate` review or ask the user for done criteria first.

---

## 1. Initial Task Briefing Template (via `agent__startSession`)

```markdown
### Goal
[Clear 1-2 sentence statement of the target deliverable]

### Scope & Boundaries
- Authorized working paths: [e.g. .libragent/work/ or src/specific_module/]
- FORBIDDEN: Do not modify any files outside the authorized paths.
- Execution mode: [e.g. shared workspace / isolated]

### Acceptance Criteria (Must verify before reporting done)
- [ ] Layer 1: [Command] exits with code 0 (e.g. `pnpm test path/to/test.ts`)
- [ ] Layer 1: Code compiles cleanly without new linter warnings
- [ ] Layer 2: No modifications outside authorized paths (`git status`)
- [ ] Layer 3: No placeholder mock functions or suppressed errors; paste raw command output as proof

### Required Deliverable
In your final text response, provide:
1. Summary of changes made (with exact file paths)
2. Raw output snippet proving acceptance criteria passed
3. Note if you hit a circuit-break, timeout, or could not finish — do not claim done
```

---

## 2. Re-steering Feedback Templates (via `agent__messageToSession`)

### Template: Layer 1 Failure (Build / Test Error)

```markdown
[Verification Failed — Layer 1: Deterministic Check]

Your reported solution failed deterministic validation.
Command executed: `cargo test test_name` (or `pnpm refactor:validate`)

Exit code: 1
Error output:
```
[Paste exact error lines from compiler or test runner]
```

Action required:
Fix the error shown above. Run the test command yourself to verify the fix before responding.
```

### Template: Layer 2 Failure (Invariant Gate / Forbidden Path Edit)

```markdown
[Verification Failed — Layer 2: Invariant Gate Violation]

You modified files outside your authorized boundary:
- Unauthorized file: `src-tauri/src/state.rs` (Protected path)

Rule:
You are only authorized to modify `.libragent/work/` and `coordination/`.

Action required:
1. Revert the changes to `src-tauri/src/state.rs` immediately (`git checkout -- <file>`).
2. Move your proposed changes into the designated output directory.
3. Confirm clean `git status` before reporting back.
```

### Template: Layer 3 Failure (Missing Evidence / Unexecuted Claims)

```markdown
[Verification Failed — Layer 3: Evidence Missing]

You claimed that "all unit tests pass", but the execution trace shows no test runner was executed.

Action required:
Run the actual verification command:
`pnpm test tests/my-test.test.ts`
Paste the raw stdout/stderr output in your response as proof.
```

### Template: Layer 3 Failure (Reliability / Incomplete Run)

```markdown
[Verification Failed — Layer 3: Reliability]

The session ended incomplete, errored, timed out, or circuit-broke. That is not an accepted completion.

Observed signal:
- [e.g. status error / empty deliverable / circuit-break guidance / cancel]

Action required:
Resume only the missing work, or report a clear blocker with evidence. Do not restate "done" without Layer 1 proof.
```

### Template: Layer 4 Failure (Semantic / Requirement Missed)

```markdown
[Verification Failed — Layer 4: Semantic Requirement Missing]

The code compiles and passes existing tests, but misses the following user requirement:
- Requirement missed: "Must handle null session IDs gracefully without panicking"

Current behavior:
In `src/foo.ts:45`, `sessionId` is unwrapped directly without null check.

Action required:
Add null handling and an explicit unit test covering the null case.
```
