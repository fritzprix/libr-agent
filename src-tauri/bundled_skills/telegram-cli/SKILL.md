---
name: telegram-cli
description: |
  Python CLI wrapper for Telegram interaction using Telethon (MTProto protocol).
  Use when the user wants to: (1) send messages, (2) read messages from chats/channels/groups,
  (3) list chats, (4) download files, or (5) search messages.
  On first use, guide the user through authentication (API ID/Hash → phone → code → 2FA password).
  Store session and config in ~/.libragent/telegram_config.json.
  Subsequent requests use the stored session without re-authentication.
  Triggers on requests like: "텔레그램 메시지 보내줘", "텔레그램 확인", "send telegram", "텔레그램 채널 확인", "텔레그램 파일 다운로드".
---

# Telegram CLI Skill

Enables the agent to interact with Telegram on behalf of the user via Telethon (MTProto protocol).

## Path conventions

Paths in this skill are relative to the directory containing this `SKILL.md`, not to the workspace root or the shell's current `./`.

- Scripts in this skill use paths like `scripts/...`
- When a command below says `python scripts/...`, resolve that script path against the skill's absolute Base Directory
- On Linux/macOS, use `python3` if `python` is unavailable
- In command examples below, replace `<skill-base-dir>` with the skill's actual absolute Base Directory

## ⚠️ Security Rules (Mandatory)

- **NEVER** ask for 2FA password, verification codes, or other transient secrets in chat
- **NEVER** display or repeat the contents of `~/.libragent/telegram_config.json`
- Collect transient secrets (verification codes, 2FA passwords) exclusively through `requireUserInput=true` shell prompts — never in chat
- For verification codes and 2FA, prefer `requireUserInput=true` with `python setup.py --code-stdin` / `--password-stdin` (LibrAgent auto-pipes UI input to child stdin on Windows when these flags are present)
- **ALWAYS** use `setup.py` to persist credentials and session
- If the user accidentally pastes a password or code in chat, acknowledge receipt, do NOT echo it back, and immediately run setup to store it properly

---

## Overview

Telegram integration involves these steps:

1. **Detect config** — run `check_config.py` to check if account is configured
2. **Setup (first time only)** — gather API ID/Hash, phone number, then run `setup.py` with hidden prompts for code/password
3. **Dispatch action** — classify the user's request and call `telegram_cli.py` with the right action
4. **Present results** — format and summarize the output for the user

---

## Prerequisites

Ensure Telethon is installed:

```bash
pip3 install telethon
```

---

## Step 1: Detect Config

Always start by checking if the account is configured:

```bash
python "<skill-base-dir>/scripts/check_config.py"
```

- Exit code `0` with `"status": "ok"` → configured and authorized, proceed to Step 3
- Exit code `1` → missing config/session or not authorized (`missing`, `missing_session`, `unauthorized`, `auth_restart_needed`), go to Step 2
- Exit code `2` → config exists but is incomplete/corrupt, go to Step 2 to reconfigure/overwrite it
- `"status": "auth_restart_needed"` → delete `~/.libragent/telegram_session.session` and restart from Step A (`send_code`)

---

## Step 2: Account Setup (First Time or Reset)

Do **not** run `python "<skill-base-dir>/scripts/setup.py"` bare inside LibrAgent. That old terminal wizard asks for multiple prompts and can time out under the current prompt-resume shell contract.

Instead:

### 2.1: Guide API ID/Hash Acquisition

If the user doesn't have API credentials, guide them to:

1. Visit https://my.telegram.org
2. Log in with their phone number
3. Click "API development tools"
4. Fill in App title and Short name (can be anything)
5. Copy the `api_id` and `api_hash`

### 2.2: Two-Step Authentication Flow

#### 1. Collect API credentials in chat

Ask the user for:
- `api_id` (integer)
- `api_hash` (string)
- `phone` (international format, e.g., `+821012345678`)

#### 2. Interactive setup commands

**Step A — Send verification code:**

```powershell
python "<skill-base-dir>/scripts/setup.py" `
  --api-id 12345678 `
  --api-hash "abcdef0123456789..." `
  --phone "+821012345678" `
  --action send_code
```

This outputs a JSON confirmation that the code was sent. On `AuthRestartError`, the script clears the partial session and retries once. If it still fails, you get `"status": "auth_restart_needed"` — run `send_code` again.

**Step B — Sign in with verification code:**

Execute `runInPersistentShell` (or `runInPersistentPowerShell`) with:

- `requireUserInput=true`
- `inputType=text`
- `inputPrompt=텔레그램 인증 코드를 입력하세요:`
- Command:

  ```powershell
  python "<skill-base-dir>/scripts/setup.py" --action sign_in --code-stdin
  ```

LibrAgent auto-detects `--code-stdin` and pipes the UI input into Python stdin (`stdinDelivery=child`).

**Fallback** if piping is unavailable on an older build:

```powershell
$code = Read-Host; python "<skill-base-dir>/scripts/setup.py" --action sign_in --code-value $code
```

**Step C — Handle 2FA (if Step B returns `"password_needed"`):**

- `requireUserInput=true`
- `inputType=password`
- `inputPrompt=텔레그램 2FA 비밀번호를 입력하세요:`
- Command:

  ```powershell
  python "<skill-base-dir>/scripts/setup.py" --action sign_in --password-stdin
  ```

After successful setup, re-run `check_config.py` to confirm `"status": "ok"`, then proceed to Step 3.

---

## Step 3: Dispatch Action

Classify the user's request into one of six actions and call `telegram_cli.py`.

### Output handling (recommended for message/search actions)

On Windows, large JSON payloads can break in PowerShell (`cp949`). Prefer saving results to a UTF-8 file:

```powershell
python "<skill-base-dir>/scripts/telegram_cli.py" --action get_messages `
  --chat "<chat_id_or_username>" `
  --limit 50 `
  --output "<workspace>/telegram_messages.json"
```

When `--output` is set, stdout prints a compact summary (`status`, `output` path, `count`). Read the file for full message bodies.

All CLI output uses UTF-8 (`ensure_ascii=False`). Errors go to stderr as UTF-8 JSON.

### Action: `send_message` — Send a message

```powershell
python "<skill-base-dir>/scripts/telegram_cli.py" --action send_message `
  --chat "<chat_id_or_username>" `
  --message "메시지 내용" `
  [--file "/path/to/attachment"]
```

Use when: "텔레그램 메시지 보내줘", "텔레그램으로 알려줘", "send telegram"

The `--chat` can be:

- Username: `@username` or `username`
- Chat ID: `-1001234567890` (group/channel) or `123456789` (user)
- `me` for self (saved messages)

For media files, include `--file` with the absolute path.

---

### Action: `get_messages` — Read recent messages

```powershell
python "<skill-base-dir>/scripts/telegram_cli.py" --action get_messages `
  --chat "<chat_id_or_username>" `
  [--limit N] `
  [--offset_id N]
```

Use when: "텔레그램 확인", "텔레그램 메시지 보여줘", "read telegram"

Default: `--limit 20 --offset_id 0`

`--offset_id` is the message ID to paginate from (older messages appear before that ID). `--offset_offset` remains supported as a deprecated alias.

---

### Action: `list_chats` — List all chats/channels/groups

```bash
python "<skill-base-dir>/scripts/telegram_cli.py" --action list_chats
```

Use when: "텔레그램 채팅 목록", "텔레그램 채널 확인", "list telegram chats"

---

### Action: `search_messages` — Search messages

```powershell
python "<skill-base-dir>/scripts/telegram_cli.py" --action search_messages `
  --query "검색어" `
  [--limit N] `
  [--chat "<chat_id_or_username>"] `
  [--offset_id N] `
  [--output "<workspace>/telegram_search.json"]
```

Use when: "텔레그램에서 ~ 검색", "search telegram"

`--offset_id` paginates older matches (same semantics as `get_messages`). `--offset_offset` remains a deprecated alias.

---

### Action: `download_file` — Download a file from a message

```powershell
python "<skill-base-dir>/scripts/telegram_cli.py" --action download_file `
  --chat "<chat_id_or_username>" `
  --message_id N `
  [--dest "/path/to/destination"]
```

Use when: "텔레그램 파일 다운로드", "download telegram file"

---

### Action: `get_chat_info` — Get chat/channel info

```bash
python "<skill-base-dir>/scripts/telegram_cli.py" --action get_chat_info \
  --chat "<chat_id_or_username>"
```

Use when: "텔레그램 채널 정보", "get telegram channel info"

---

## AI Agent Usage Guide

This section is for **how to run commands after setup** — not what to implement. Follow it whenever dispatching `telegram_cli.py`.

### Standard workflow

1. Run `check_config.py` — stop and re-auth if not `"status": "ok"`.
2. If the user gave a chat **name** (not `@username` / numeric ID), run `list_chats` first and pick the matching `id` or `username`.
3. Run the target action with `--output <workspace>/telegram_<action>.json` for message/search/list payloads.
4. Read the JSON file (not raw stdout) and format results for the user.
5. If the user asks for more, paginate with `next_offset_id` (see below).

**Chat ID resolution order:** `list_chats` → match by `name` / `username` → use numeric `id` or `@username` in `--chat` → `get_chat_info` if metadata is needed.

### Usage examples (copy-ready)

Replace `<skill-base-dir>` with this skill's absolute path and `<workspace>` with the agent session workspace.

**List chats (find a channel by name):**

```powershell
python "<skill-base-dir>/scripts/telegram_cli.py" --action list_chats `
  --output "<workspace>/telegram_chats.json"
```

**Read messages (first page):**

```powershell
python "<skill-base-dir>/scripts/telegram_cli.py" --action get_messages `
  --chat "재신 이" `
  --limit 50 `
  --output "<workspace>/telegram_messages_p1.json"
```

**Read messages (next page — use `next_offset_id` from the previous file):**

```powershell
python "<skill-base-dir>/scripts/telegram_cli.py" --action get_messages `
  --chat "재신 이" `
  --offset_id 12345 `
  --limit 50 `
  --output "<workspace>/telegram_messages_p2.json"
```

**Search messages (first page):**

```powershell
python "<skill-base-dir>/scripts/telegram_cli.py" --action search_messages `
  --query "이재신" `
  --limit 50 `
  --output "<workspace>/telegram_search_p1.json"
```

**Search messages (scoped to one chat + pagination):**

```powershell
python "<skill-base-dir>/scripts/telegram_cli.py" --action search_messages `
  --query "이재신" `
  --chat "재신 이" `
  --offset_id 12345 `
  --limit 50 `
  --output "<workspace>/telegram_search_p2.json"
```

**Send message:**

```powershell
python "<skill-base-dir>/scripts/telegram_cli.py" --action send_message `
  --chat "@example_channel" `
  --message "안녕하세요"
```

**Download attachment from a message:**

```powershell
python "<skill-base-dir>/scripts/telegram_cli.py" --action download_file `
  --chat "재신 이" `
  --message_id 67890 `
  --dest "<workspace>/downloads"
```

### `--output` behavior

| Mode | stdout | Full payload |
| ---- | ------ | ------------ |
| Without `--output` | Full JSON | stdout |
| With `--output path` | Compact summary JSON | UTF-8 file at `path` |

Compact stdout example:

```json
{"status":"ok","output":"C:\\workspace\\telegram_messages_p1.json","action":"get_messages","count":50,"chat":"재신 이","offset_id":0,"next_offset_id":12345}
```

**Always read the file** when `--output` is set. Do not parse large message arrays from stdout on Windows.

### Output schemas

#### `get_messages` / `search_messages` (full JSON in file or stdout)

```json
{
  "status": "ok",
  "action": "get_messages",
  "chat": "재신 이",
  "count": 50,
  "offset_id": 0,
  "next_offset_id": 12345,
  "messages": [
    {
      "id": 200,
      "date": "2025-10-22T14:30:00+00:00",
      "text": "메시지 내용",
      "has_media": false,
      "from": "재신 이"
    }
  ]
}
```

- `offset_id` — cursor used for **this** request (0 = newest page).
- `next_offset_id` — pass as `--offset_id` on the **next** request; `null` when no more pages (`count < limit`).
- `search_messages` adds `"query": "..."` and optional `"chat": "..."`.
- Media fields when present: `file_name`, `file_size`, `mime_type`, or `"photo": true`.

#### `list_chats`

```json
{
  "status": "ok",
  "action": "list_chats",
  "count": 25,
  "chats": [
    {
      "id": -1001234567890,
      "name": "재신 이",
      "type": "channel",
      "username": "example_channel",
      "subscriber_count": 1200
    }
  ]
}
```

#### `send_message`

```json
{
  "status": "ok",
  "action": "send_message",
  "message_id": 123,
  "chat": "@example_channel",
  "sent_at": "2026-06-08T12:00:00+00:00"
}
```

#### `download_file`

```json
{
  "status": "ok",
  "action": "download_file",
  "file": "C:\\workspace\\downloads\\report.pdf",
  "size": 102400,
  "message_id": 67890
}
```

#### Error (stderr, any action)

```json
{
  "status": "error",
  "message": "Chat not found: unknown"
}
```

### Exit codes

| Code | Meaning | Agent action |
| ---- | ------- | ------------ |
| `0` | Success | Parse output / read `--output` file |
| `1` | Config, auth, or generic failure | Run `check_config.py`; re-auth if needed |
| `2` | Telegram API error (incl. FloodWait, chat not found) | See error table below |
| `3` | Missing CLI arguments | Fix command and retry |

### Pagination workflow

```
1. get_messages --chat X --limit 50 --output page1.json
2. Open page1.json → read next_offset_id (e.g. 12345)
3. If next_offset_id is null → done
4. get_messages --chat X --offset_id 12345 --limit 50 --output page2.json
5. Repeat until next_offset_id is null
```

Same pattern for `search_messages` (include `--query` and optional `--chat` on every page).

### Error handling (agent actions)

| Error / pattern | Detection | What the agent should do |
| --------------- | --------- | ------------------------ |
| Chat not found | `"message": "Chat not found: ..."` (exit `2`) | Run `list_chats --output chats.json`, find correct `id`/`username`, retry |
| FloodWait | `"message": "FloodWait: wait N seconds"` (exit `2`) | Wait **N** seconds (shell `sleep` / `Start-Sleep`), retry same command |
| Not authorized | `"Not authorized. Run setup first."` (exit `1`) | Run `check_config.py`; if not ok, run Step 2 auth flow |
| Auth restart | `check_config` → `auth_restart_needed` | Delete `~/.libragent/telegram_session.session`, `send_code` → `sign_in` |
| Missing `--chat` / `--query` | exit `3` | Add required flag and retry |
| cp949 / garbled stdout | Mojibake in console | Re-run with `--output` and read the UTF-8 file |
| Message has no file | `"Message has no downloadable file"` | Tell user; try another `message_id` |
| File not found (send) | `"File not found: ..."` | Verify attachment path in workspace |

Never paste verification codes or 2FA passwords in chat — use `requireUserInput` + `setup.py` only.

---

## Step 4: Present Results

- **Message list**: Show as a numbered table — `#  From  Date  Content`
- **Send confirmation**: Confirm action completed with brief summary
- **Chat list**: Show as a table — `#  Name  Type  Members/ID  Last Activity`
- **Search results**: Same as message list, with match count
- **Errors**: See Error Handling below

---

## Error Handling (setup & auth)

For runtime `telegram_cli.py` errors, use the **Error handling (agent actions)** table in [AI Agent Usage Guide](#ai-agent-usage-guide).

| Error                | Cause                         | Action                                            |
| -------------------- | ----------------------------- | ------------------------------------------------- |
| Auth required        | Session expired or invalid    | Re-run Step 2's authentication flow               |
| `auth_restart_needed`| Telegram auth state corrupted | Delete session file, run `send_code` again        |
| Unauthorized session | `send_code` only, no sign_in  | Complete Step B/C (`sign_in`)                       |
| ambiguous `--code`   | Old docs used `--code`        | Use `--code-value`, `--code-env`, or `--code-stdin` |
| stdin not received   | Old builds on Windows       | Upgrade LibrAgent or use Read-Host + `--code-value` fallback |
| API hash invalid     | Wrong or revoked API hash     | Re-run Step 2 with correct credentials            |
| Phone number invalid | Format error                  | Guide user to enter international format (+82...) |

Always provide the next concrete step, never just report the error.

---

## Output Format Guidelines

For message listings, use this format:

```
📨 텔레그램 메시지 (chat: @example_channel)

#  날짜              내용
1  06/01 14:23      오늘 회의는 14시에 시작됩니다.
2  06/01 13:45      [이미지]
3  06/01 12:00      새로운 기능 배포 완료

더 보려면: "다음 20개 보여줘"
```

For send confirmation:

```
✅ 텔레그램 메시지 발송 완료
  받는 곳: @example_channel
  발송 시각: 2026-06-01 14:30
```

For chat listings:

```
💬 텔레그램 채팅 목록 (총 25개)

#  이름                    유형       마지막 활동
1  ● LibrAgent Dev         채널       10분 전
2    GitHub Notifications  채널       1시간 전
3  ● 프로젝트 A            그룹(42)   3시간 전
4    김철수                개인       어제

● = 읽지 않음
```
