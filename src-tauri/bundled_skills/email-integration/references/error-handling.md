# Error Handling

| Error | Cause | Action |
|---|---|---|
| Auth failed (IMAP 535) | Wrong password or App Password needed | Re-run setup with `--reset` using the hidden-prompt + visible setup flow; guide user to provider App Password docs |
| Connection refused | Wrong host/port | Check [server-profiles.md](server-profiles.md), re-collect four server fields, re-run setup |
| Config not found | First run or deleted | Run non-interactive setup; do **not** launch the bare wizard |
| SSL error | Port mismatch | Suggest switching 993↔143 or 587↔465 |
| Send failed (SMTP 550) | Recipient rejected | Confirm address with user |

Always provide the next concrete step—never just report the error.
