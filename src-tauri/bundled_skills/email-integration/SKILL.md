---
name: email-integration
description: |
  Integrates with any IMAP/SMTP mail server (Gmail, Outlook, Naver, Kakao, custom servers, etc.).
  Use when the user wants to read, send, search, reply to, or manage emails.
  On first use, collect the email/server settings, capture the password through a single hidden shell prompt, and run setup_account.py non-interactively to store credentials in a local config file with restricted permissions.
  Subsequent requests use the stored config without re-asking for credentials.
  Triggers on requests like: "메일 보여줘", "이메일 확인", "send email", "받은 편지함", "메일 보내줘", "메일 검색".
---

# Email Integration

Interact with any IMAP/SMTP mail server on behalf of the user.

## Path conventions

Paths are relative to this skill's Base Directory. Replace `<skill-base-dir>` with its absolute path in commands.

## Security (mandatory)

- **NEVER** ask for passwords in chat or display `~/.libragent/email_config`
- Collect password only via a single hidden shell prompt; persist with `setup_account.py`
- If the user pastes a secret in chat, do not echo it—run setup immediately

## Workflow

### 1. Detect config

```bash
python "<skill-base-dir>/scripts/validate_config.py"
```

- Exit `0` → configured, go to step 3
- Exit `1` → not configured, go to step 2
- Exit `2` → corrupt config, go to step 2 with `--reset`

### 2. Setup (first time or reset)

See [references/setup-flow.md](references/setup-flow.md) for the full PowerShell two-step pattern and preset/custom domain commands.

### 3. Dispatch action

Classify the request and call `email_client.py`:

| Action | When | Command |
|---|---|---|
| `read_inbox` | Browse inbox | `--action read_inbox [--limit N] [--filter unread\|today\|all]` |
| `read_email` | Full content by UID | `--action read_email --uid <UID>` |
| `send_email` | Compose/send/reply | `--action send_email --to ... --subject ... --body ... [--reply-to-uid <UID>]` |
| `search_email` | Keyword/sender/date | `--action search_email --query "<IMAP search>"` |
| `manage_email` | Mark/move/delete | `--action manage_email --uid <UID> --op <mark_read\|mark_unread\|move\|delete>` |

Search query examples: see [references/request-patterns.md](references/request-patterns.md).
Server presets: see [references/server-profiles.md](references/server-profiles.md).

Draft vague compose requests yourself; confirm with the user before sending.
For bulk delete/move, confirm count first.

### 4. Present results

See [references/output-format.md](references/output-format.md). Errors: [references/error-handling.md](references/error-handling.md).
