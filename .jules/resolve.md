# Resolve - Debt Repayment Log

This log tracks technical debt repayment: TODOs completed, FIXMEs resolved, and HACKs replaced with proper solutions.

## Format
`## YYYY-MM-DD - [File] **Debt Cleared:** [Original Comment] **Solution:** [Implementation Summary]`

## 2025-05-18 - src/lib/backend/session-crud.ts **Debt Cleared:** `// TODO: extract from assistants` **Solution:** Extracted unique `mcpServerIds` from session assistants and populated the `mcpServerIds` field in `AgentConfig` payload, matching the backend expectation.
