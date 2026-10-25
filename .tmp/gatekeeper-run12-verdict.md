# Gatekeeper Run #12 — PR #2220 Verdict

**PR**: #2220 — `feat(workspace): reject shell syntax errors before running runShell`
**Branch**: `feat/run-shell-syntax-precheck-2214` → `dev/0.9.x`
**Reviewer**: Gatekeeper (Coding Expert)

## Changes

| File                          | Status   | Lines |
| ----------------------------- | -------- | ----- |
| `syntax.rs` (new)             | Added    | +251  |
| `handlers.rs`                 | Modified | +19   |
| `guidance.rs`                 | Modified | +1/-1 |
| `mod.rs`                      | Modified | +2    |
| `workspace_guidance_tests.rs` | Modified | +36   |

## Verification

| Criterion                     | Check                                  | Result                                             |
| ----------------------------- | -------------------------------------- | -------------------------------------------------- |
| **C1: Compilation**           | `cargo check` + `cargo clippy --tests` | ✅ Pass (clippy clean, only pre-existing warnings) |
| **C2: No `any` casts**        | No TypeScript changes in this PR       | ✅ N/A (Rust only)                                 |
| **C3: No blocking `std::fs`** | Grep `syntax.rs` for `std::fs`         | ✅ Zero calls — uses `tokio::process::Command`     |
| **C4: Tests**                 | Unit tests + integration test          | ✅ 6 unit tests + 1 integration test = 7 total     |
| **C5: KISS/YAGNI**            | Single-purpose module, focused scope   | ✅ 251 lines for `bash -n` pre-check — reasonable  |

## Analysis

### What it does

Runs `bash -n -c <command>` (syntax-only check) before executing `runShell`/`runInPersistentShell`. If bash reports a parse error (unclosed quotes, EOF on unterminated block), returns a guided MCP error with:

- Error category: `InvalidInput`
- Guidance: Suggests `writeFile` to fix the command
- Details: Line number, error message, unmatched quote character, offending snippet

### Security review

- Uses `env_remove("BASH_ENV")` + `env_remove("ENV")` to prevent sourcing
- Command passed as `.arg(command)` to `bash -n -c` — safe, no shell interpolation
- 3-second timeout — prevents hanging
- Fails open (`None`) if bash unavailable — acceptable per doc comment
- Windows already has separate `validate_windows_shell_syntax()` ✅

### Code quality

- `BashSyntaxError` struct is well-structured with typed fields
- Regex parsing of bash error output is clean and documented
- `test_syntax_check_is_sandboxed` verifies commands are not executed — good security test

## Verdict

### [APPROVED]

All criteria satisfied. Clean addition of bash syntax pre-check. 945 integration tests pass, 0 failures.
