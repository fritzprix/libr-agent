---
name: email-integration
description: |
  Integrates with any IMAP/SMTP mail server (Gmail, Outlook, Naver, Kakao, custom servers, etc.).
  Use when the user wants to read, send, search, reply to, or manage emails.
  On first use, collect the email/server settings, capture the password through a single hidden shell prompt, and run setup_account.py non-interactively to store credentials in a local config file with restricted permissions.
  Subsequent requests use the stored config without re-asking for credentials.
  Triggers on requests like: "메일 보여줘", "이메일 확인", "send email", "받은 편지함", "메일 보내줘", "메일 검색".
---

# Email Integration Skill

Enables the agent to interact with any IMAP/SMTP mail server on behalf of the user.

## Path conventions

Paths in this skill are relative to the directory containing this `SKILL.md`, not to the workspace root or the shell's current `./`.

- Scripts in this skill use paths like `scripts/...`
- Reference material in this skill uses paths like `references/...`
- When a command below says `python scripts/...`, resolve that script path against the skill's absolute Base Directory
- In command examples below, replace `<skill-base-dir>` with the skill's actual absolute Base Directory
- Files like config exports or attachments are workspace/user files unless the instruction explicitly says otherwise

## ⚠️ Security Rules (Mandatory)

- **NEVER** ask for passwords, app passwords, or other secrets in chat
- **NEVER** display or repeat the contents of `~/.libragent/email_config`
- Ask for the email address and custom server overrides in chat only when needed; collect the password only through a hidden shell prompt
- **ALWAYS** use `setup_account.py` to persist credentials
- If the user accidentally pastes a password or other secret in chat, acknowledge receipt, do NOT echo it back, and immediately run setup to store it properly

---

## Overview

Email integration involves these steps:

1. **Detect config** — run `validate_config.py` to check if account is configured
2. **Setup (first time only)** — gather non-secret setup fields, then run `setup_account.py` with args plus one hidden password prompt
3. **Dispatch action** — classify the user's request and call `email_client.py` with the right action
4. **Present results** — format and summarize the output for the user

---

## Step 1: Detect Config

Always start by checking if the account is configured:

```bash
python "<skill-base-dir>/scripts/validate_config.py"
```

- Exit code `0` → configured, proceed to Step 3
- Exit code `1` → not configured, go to Step 2
- Exit code `2` → config exists but is incomplete/corrupt, go to Step 2 with `--reset` flag

---

## Step 2: Account Setup (First Time or Reset)

Do **not** run `python "<skill-base-dir>/scripts/setup_account.py"` bare inside LibrAgent. That old terminal wizard asks for multiple prompts and can time out under the current prompt-resume shell contract.

Instead:

1. Determine the email address.
2. For known preset domains from `references/server-profiles.md`, use the preset flow.
3. For custom/self-hosted domains, including corporate Microsoft 365 domains outside `*.onmicrosoft.com`, ask for all four values in chat first: IMAP host/port and SMTP host/port.
4. Collect the password through a **single hidden shell prompt**, store it in a temporary environment variable in the persistent shell, then run `setup_account.py` in a separate visible command that reuses that variable.

### Preferred PowerShell pattern in LibrAgent

Run `runInPersistentPowerShell` with:

- `requireUserInput=true`
- `inputType=password`
- an `inputPrompt` like `이메일 비밀번호 또는 앱 비밀번호를 입력하세요:`
- an explicit setup timeout such as `timeout=90`

Use a **two-step PowerShell flow** in LibrAgent:

1. Hidden prompt call: set the temporary password environment variable.
2. Visible setup call: run `setup_account.py ... --password-env ...` with no hidden input so the script's non-secret diagnostics remain visible.
3. Cleanup call: remove the temporary environment variable.

Use `--password-env` for this two-step persistent-shell pattern. If the runtime can pipe the hidden password directly into the setup command's stdin without exposing it, `--password-stdin` is a more private alternative.

Hidden prompt command:

```powershell
$env:LIBRAGENT_EMAIL_PASSWORD = Read-Host
```

This `Read-Host` snippet is safe **only** when LibrAgent is enforcing hidden password input for that call. Running it directly in a normal terminal will echo the password on screen.

Visible setup command for known preset domains:

```powershell
python "<skill-base-dir>/scripts/setup_account.py" `
  --email "user@example.com" `
  --use-preset-servers `
  --password-env LIBRAGENT_EMAIL_PASSWORD
```

Visible setup command for reset after auth failure:

```powershell
python "<skill-base-dir>/scripts/setup_account.py" `
  --reset `
  --email "user@example.com" `
  --use-preset-servers `
  --password-env LIBRAGENT_EMAIL_PASSWORD
```

Visible setup command for custom/self-hosted domains:

```powershell
python "<skill-base-dir>/scripts/setup_account.py" `
  --email "user@example.com" `
  --imap-host "imap.example.com" `
  --imap-port 993 `
  --smtp-host "smtp.example.com" `
  --smtp-port 587 `
  --password-env LIBRAGENT_EMAIL_PASSWORD
```

Cleanup command:

```powershell
Remove-Item Env:LIBRAGENT_EMAIL_PASSWORD -ErrorAction SilentlyContinue
```

On non-PowerShell shells, use the same two-step pattern: collect one hidden password in the shell, export it temporarily, run `setup_account.py --password-env ...` in a separate visible command, then clear it. If the shell/tool runtime can securely pipe the hidden password straight into stdin, use `setup_account.py --password-stdin` instead of a temporary environment variable.

After successful setup, proceed to Step 3 with the original user request.

---

## Step 3: Dispatch Action

Classify the user's request into one of five actions and call `email_client.py`:

### Action: `read_inbox` — Browse received emails

```bash
python "<skill-base-dir>/scripts/email_client.py" --action read_inbox [--limit N] [--filter unread|today|all]
```

Use when: "메일 보여줘", "받은 편지함", "최근 메일", "안 읽은 메일"

Default: `--limit 10 --filter unread`

---

### Action: `read_email` — Read full email content

```bash
python "<skill-base-dir>/scripts/email_client.py" --action read_email --uid <UID>
```

Use when: user refers to a specific email by number/index from a previous list, or "이 메일 내용 보여줘"

The UID comes from a prior `read_inbox` call. Always mark as read after displaying.

---

### Action: `send_email` — Compose and send

```bash
python "<skill-base-dir>/scripts/email_client.py" --action send_email \
  --to "recipient@example.com" \
  --subject "제목" \
  --body "본문" \
  [--cc "cc@example.com"] \
  [--reply-to-uid <UID>]
```

Use when: "메일 보내줘", "답장 써줘", "이메일 작성"

For replies, always include `--reply-to-uid` so `In-Reply-To` and `References` headers are set correctly.

**Compose guidance**: If the user gives a vague request ("김 팀장한테 회의 일정 잡는 메일"), draft the full body yourself and confirm with the user before sending.

---

### Action: `search_email` — Search by keyword/sender/date

```bash
python "<skill-base-dir>/scripts/email_client.py" --action search_email --query "<IMAP search string>"
```

Query construction examples:
- "김철수한테 온 거" → `--query "FROM 김철수"`
- "지난주 메일" → `--query "SINCE 26-May-2026"`
- "프로젝트 관련" → `--query "SUBJECT 프로젝트"`
- "첨부파일 있는 거" → `--query "BODY attachment"` (best effort)

Refer to `references/request-patterns.md` for more examples.

---

### Action: `manage_email` — Mark/move/delete

```bash
python "<skill-base-dir>/scripts/email_client.py" --action manage_email \
  --uid <UID> \
  --op <mark_read|mark_unread|move|delete> \
  [--folder <destination_folder>]
```

Use when: "읽음 처리", "삭제해줘", "스팸함으로 이동", "보관함으로"

For bulk operations (e.g. "스팸함 비워"), confirm count with user before proceeding.

---

## Step 4: Present Results

- **Inbox list**: Show as a numbered table — `#  From  Subject  Date  (Unread?)`
- **Email body**: Show full content, list attachments separately
- **Search results**: Same as inbox list, with match count
- **Send/manage**: Confirm action completed with brief summary
- **Errors**: See Error Handling below

---

## Error Handling

| Error | Cause | Action |
|---|---|---|
| Auth failed (IMAP 535) | Wrong password or App Password needed | Re-run Step 2 with `--reset` using the hidden-prompt + visible setup flow, then guide the user to the provider's App Password docs |
| Connection refused | Wrong host/port | Check `references/server-profiles.md`, re-collect the four server fields in chat, then re-run Step 2 with an explicit setup timeout |
| Config not found | First run or deleted | Run Step 2's non-interactive setup flow; do **not** launch the old bare wizard |
| SSL error | Port mismatch | Suggest switching 993↔143 or 587↔465 |
| Send failed (SMTP 550) | Recipient rejected | Confirm address with user |

Always provide the next concrete step, never just report the error.

---

## Output Format Guidelines

For inbox listings, use this format:

```
📬 받은 편지함 (읽지 않은 메일: 3 / 총 47개)

#  보낸 사람              제목                          날짜
1  ● 김철수 <k@corp.com>  [긴급] 서버 점검 안내         오늘 09:14
2  ● 박영희 <p@corp.com>  6월 회의 일정 공유            오늘 08:30
3    noreply@github.com   PR #142 merged                어제 23:11

● = 읽지 않음  |  더 보려면: "다음 10개 보여줘"
```

For send confirmation:

```
✅ 메일 발송 완료
  받는 사람: recipient@example.com
  제목: 회의 일정 확인 요청
  발송 시각: 2026-06-01 14:23
```
