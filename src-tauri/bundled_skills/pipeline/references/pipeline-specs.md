# Pipeline Handover Specs

Clear handover rules prevent information loss and context bloat across pipeline stages.

## 📌 Handover Rules

1. **File-Centric Handover**
   - Do not pass large code chunks or text blocks directly inside tool arguments.
   - Pass relative or absolute **file paths** inside the shared workspace.
   
2. **Contextual Summary**
   - Provide a brief summary of what was completed and list outstanding checklist items (TODOs) for the next stage.

3. **State Isolation**
   - Each stage runs with isolated conversation/runtime state to prevent prompt memory cross-contamination. An Idle child with the same assistant configuration may be reused with `reset=true`; only share physical files and summary prompts.

---

## 📋 Example Configuration

### Software Release Pipeline (Research → Implement → Document → Verify)

```
[Stage 1] Research (Researcher Assistant)
  - Task: "Compare library alternatives, recommend the best candidate, and draft the API design doc."
  - Output: `docs/design_v1.md`

[Stage 2] Implement (Coding Assistant)
  - Task: "Implement the Rust module following docs/design_v1.md."
  - Input: `docs/design_v1.md`
  - Output: `src/analyzer.rs`

[Stage 3] Document (Doc Assistant)
  - Task: "Draft a usage guide and README based on src/analyzer.rs."
  - Input: `src/analyzer.rs`
  - Output: `README.md`

[Stage 4] Verify (QA Assistant)
  - Task: "Validate compilation and write integration test scripts for src/analyzer.rs."
  - Input: `src/analyzer.rs`
  - Output: Test execution log summary
```

---

## 🚫 Error Prevention

- **Stop on Failure:** If a stage fails or errors out, stop spawning the next stages, report the failure to the parent session, and wait for human input.
- **File Versioning:** Clarify in the prompt whether a stage should overwrite the existing file or write to a new path (e.g., `file_draft.md` vs `file_final.md`).
