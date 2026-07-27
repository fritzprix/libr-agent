# Code audit report template

Write the report to `.libragent/work/code_audit_report.md` (or another `.libragent/work/*` path). Match the user’s language for prose.

---

## 1. Executive summary

- **Subject**: (what was audited)
- **Stated goal**: (from PR / agent summary)
- **Verdict**: (pass / pass-with-risks / fail) — one sentence why
- **Overall score**: only if ≥2 axes were scored; else omit

## 2. Claims vs code

| Claim (from summary/PR) | Evidence (path:line or “not found”) | Result |
| ----------------------- | ----------------------------------- | ------ |
|                         |                                     | match / mismatch / unverified |

If nothing to contradict: one row “No material mismatches found” with the files you opened.

## 3. Work analysis

### [Change A: short name]

- **What changed**:
- **Design intent**:
- **Effect** (observed, not speculated):

Repeat per distinct change. Skip fluff.

## 4. Quality axes

Score **1–5 only for relevant axes**. Use `N/A` + reason otherwise. Never invent a high score to fill the table.

| Axis | Score | Feedback (cite path:line) |
| ---- | :---: | ------------------------- |
| Modularity | | |
| Interface design (ISP) | | |
| DRY | | |
| Cost (tokens / caching) | | |
| Reliability | | |

Add a custom row if the change is about something else (e.g. pagination UX, schema clarity). Drop unused rows.

## 5. Risks and follow-ups

- **Side effects**:
- **Tech debt left behind**:
- **Next actions** (ordered, concrete):

## 6. Conclusion

2–4 sentences. No restating the whole matrix. End with the single most important next step if any.
