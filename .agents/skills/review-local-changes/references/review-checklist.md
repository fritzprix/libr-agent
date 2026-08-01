# Local Code Change Review Checklist

Use this checklist when inspecting local uncommitted code changes to ensure high quality, safety, and maintainability.

---

## 1. Correctness & Logical Integrity

- [ ] **Functional Alignment**: Do the changes fulfill the user's explicit request or intent without unintended side effects?
- [ ] **Edge Cases**: Are null/undefined values, empty arrays, zero/negative bounds, and non-happy-path branches properly handled?
- [ ] **Async & Concurrency**: Are promises properly awaited? Are race conditions, unhandled rejections, or floating promises avoided?
- [ ] **State & Scope**: Are variable scopes tight and state mutations predictable?

---

## 2. Type Safety & API Contracts

- [ ] **Strict Typing**: Are explicit types used instead of `any` or loose assertions?
- [ ] **Schema Validation**: If parsing external data (JSON, IPC, HTTP, environment variables), is schema runtime validation (e.g., Zod) applied?
- [ ] **Interface & Command Boundaries**: Do frontend-to-backend commands (e.g., Tauri commands, Rust Results) match interface definitions strictly?
- [ ] **Option / Result Handling**: In Rust or TypeScript, are `Result`/`Option`/`Error` outcomes handled explicitly without unwrap/panic in non-test code?

---

## 3. Leftovers & Cleanliness

- [ ] **Debug Artifacts**: Are temporary `console.log`, `print`, `dbg!`, `println!`, or temporary test logs removed or replaced with proper logger calls?
- [ ] **Dead & Commented-out Code**: Is obsolete or commented-out code cleaned up?
- [ ] **Hardcoded Values & Secrets**: Are secrets, API keys, tokens, or local machine absolute paths kept out of source files?
- [ ] **Unused Imports / Variables**: Are unused imports or declared variables pruned?

---

## 4. Security & Safety

- [ ] **Injection Prevention**: Are shell commands, SQL queries, or HTML/DOM insertions safely parameterized or sanitized?
- [ ] **Path Traversal & Boundaries**: Are file system paths validated against path traversal (e.g. `..` escapes) or restricted workspace scopes?
- [ ] **Sensitive Data Exposure**: Are sensitive fields excluded from error logs, UI error states, or persistent context?

---

## 5. Performance & Resource Management

- [ ] **Re-render Optimization**: In React components, are expensive calculations, hooks dependencies, and event handlers optimized?
- [ ] **Resource Cleanup**: Are process handles, network sockets, timers, or event listeners cleaned up appropriately?
- [ ] **Database & I/O Efficiency**: Are redundant database calls or excessive file system reads minimized?

---

## 6. Testing & Project Health

- [ ] **Validation Commands**: Do relevant lint, formatting, type check, or build commands pass (e.g., `pnpm lint`, `pnpm build`, `cargo clippy`, `cargo test`)?
- [ ] **Test Coverage**: Are new features or modified edge cases covered by unit/integration tests where applicable?
