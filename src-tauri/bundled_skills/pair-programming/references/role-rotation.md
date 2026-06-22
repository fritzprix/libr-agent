# Driver-Navigator Role Rotation

Guidelines for turn execution and role-swapping in the Pair Programming workflow.

## 📌 Role Definitions

* **Driver**
   - **Allowed Actions:** File writes and edits (`write_file`, `replace_file_content`).
   - **Focus:** Implementation logic, translating Navigator's guidelines into code.
   
* **Navigator**
   - **Allowed Actions:** Read-only actions (`view_file`, `list_dir`, `grep_search`).
   - **Focus:** Architectural patterns, edge cases, error detection, and task list navigation.

---

## 🔄 Role Rotation Triggers

Swap Driver and Navigator roles under these conditions:

1. **TDD Swap**
   - **Navigator** switches to Driver to write tests.
   - **Driver** switches to Navigator to review tests, then swaps back to implement the code to pass them.

2. **Domain Swap**
   - When shifting from backend (Rust) to frontend (React), swap roles if one model is specialized in React and the other in Rust.

3. **Turn Cap**
   - If one session has acted as the Driver for **5** consecutive turns, force a swap to ensure thorough cross-verification.

---

## 🚫 Safe Pair Practices

- **Strict Write Serialization:** Never allow both sessions to edit the workspace concurrently. Only the active Driver may execute write tools.
- **Navigator Constraint:** Navigators must not write code directly. They must guide via instruction logs, letting the Driver implement the edits.
