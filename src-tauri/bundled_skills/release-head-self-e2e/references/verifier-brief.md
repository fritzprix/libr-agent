# Verifier child brief (template)

Independent from executor prose. Parent passes **artifact paths only**.

---

You are the **verifier** for matrix case **{{CASE_ID}}** (run `{{RUN_ID}}`).

## Rules

- Do **not** trust executor final text alone. Read files on disk and, when present, rerun lightweight checks (`pytest` snippets, grep rules without printing secret values).
- Verdict **pass** only with objective evidence. **blocked** if platform/driver missing. **fail** with diagnosis.
- Negative controls when case requires: E2E HTTP 404 without `LIBRAGENT_E2E_CONTROL`; wrong capability header → reject; stale execution id → reject; synthetic secret must not appear in logs you are allowed to read (metadata-only keychain/cmdkey listings OK).

## Inputs

- `cases/{{CASE_ID}}/evidence/` (from executor)
- `benchmarks/harbor/libragent-session-e2e-matrix/shared/verifier_lib.py` patterns when applicable
- Matrix id **{{CASE_ID}}** expectation from issue #2565 / `plan.md`

## Output

Write `.libragent/work/release-head-self-e2e/{{RUN_ID}}/cases/{{CASE_ID}}/verifier-verdict.md`:

```
verdict: pass|fail|blocked
evidence:
  - <path>
diagnosis: ...
negative_controls_run: ...
```

Final assistant text must repeat verdict + evidence list only (no secret values).
