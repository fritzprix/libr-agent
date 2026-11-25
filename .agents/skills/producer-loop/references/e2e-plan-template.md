# e2e-plan items template

The harness follows [../../libr-e2e-verify/references/e2e-build-precondition.md](../../libr-e2e-verify/references/e2e-build-precondition.md) before any checklist item.

`e2e_plan.py render` builds the issue body from `collected.json` and an `items.json` that you write. The body follows the #2565 format: Summary, Scope, Risk-first top 10, Checklist, Acceptance criteria, Notes. The script writes Summary, Scope, Acceptance criteria, the fixed Notes, the item IDs, and the range marker. You write only the items.

Write in English, ASD-STE100 style. Use dummy secrets only (for example `dummy-token-abcd`).

```json
{
  "top10": [
    "E-1, E-2: the password is not in the output or the logs (#2562)."
  ],
  "areas": [
    {
      "prefix": "E",
      "title": "email-integration setup (#2546 part 4)",
      "items": [
        {
          "tag": "auto",
          "text": "new session; `LIBRAGENT_EMAIL_PASSWORD` not stored → run setup_account.py with `--password-env` → exit 3 with a `storeAs` hint",
          "prs": [2562],
          "source": "PR body"
        },
        {
          "tag": "manual",
          "text": "the agent asks with `storeAs: LIBRAGENT_EMAIL_PASSWORD` → enter `dummy-token-abcd` → the dialog masks the input",
          "prs": [2562],
          "source": "issue AC"
        },
        {
          "tag": "blocked: #2557",
          "text": "a prompt waits → restart the app → record the text only",
          "prs": [2562],
          "source": "issue AC"
        }
      ]
    }
  ],
  "notes": ["Optional extra notes."]
}
```

Rendered item: `- **E-1** [auto] – <text> (#2562, source: PR body)`. IDs number from 1 in each area.

## Fields

- `prefix`: 1 to 3 capital letters, unique per area (for example `P`, `B`, `S`, `M`).
- `text`: `precondition → steps → expected result`. One check per item. The expected result must be observable.
- `prs`: the PRs that the item checks.
- `source`: `issue AC` (the linked issue acceptance criteria or requirements) or `PR body`.
- `tag`:
  - `auto`: the harness can instruct it through the Session API and judge it from the result text, the session status, or the logs.
  - `manual`: it needs a dialog answer, keyboard input, a visual check, sudo, the OS keychain, Telegram, or an app restart. The Session API has no route to answer user prompts (owner decision 2026-11-04), so any item that needs an answer is `manual`.
  - `blocked: #N`: the feature is not implemented or not merged. `#N` is the blocker issue or PR.

## Rules

- Cover each PR in Scope with at least one item.
- Put the highest-risk items first in `top10` (secret leaks, hangs, data loss).
- Do not copy secrets, private paths, or personal data from PR or issue bodies.
- Source text is not checked against the code. Do not invent expected text that the sources do not state; write "record the text" instead.
