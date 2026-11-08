# Executor child brief (template)

Paste into `agent__spawnSession` `task` / follow-up message. Replace `{{CASE_ID}}`, `{{RUN_ID}}`, paths.

---

You are the **executor** for matrix case **{{CASE_ID}}** (release→HEAD self-E2E run `{{RUN_ID}}`).

## Rules

- Unattended only. If the app needs a human-only path that the harness E2E control cannot drive (native GUI outside the app window), stop with verdict **blocked** and reason — not pass.
- Use **synthetic** secret `LIBRAGENT_E2E_SYNTHETIC_SECRET` only; never real credentials.
- Tools: exact builtins from your list (`workspace__*`, `agent__*` as needed). No invented HTTP tools except documented Session API paths when the brief requires curl.
- If a secret input or approval dialog appears while you run, the **external harness** resolves it via the debug E2E control plane. Never ask for or handle capability tokens yourself.

## Objective

{{CASE_OBJECTIVE_FROM_PLAN_MD}}

## Deliverables (write under repo or artifact path)

1. `.libragent/work/release-head-self-e2e/{{RUN_ID}}/cases/{{CASE_ID}}/executor-messages.json` — export via Session API messages if you are the API-driven child, or summarize tool results with file paths.
2. `evidence/` — workspace files, Harbor `session_e2e_evidence.json`, or verifier inputs required by the case.
3. Final text: **Verdict** `pass` | `fail` | `blocked`, **Evidence paths**, **Diagnosis** if fail.

## Cleanup

Terminate/delete **this** session when done. Do not stop the parent orchestrator session.
