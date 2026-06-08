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
- Collect transient secrets (verification codes, 2FA passwords) exclusively through interactive hidden shell prompts (`requireUserInput=true`)
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
- Exit code `1` → missing config/session or not authorized (`missing`, `missing_session`, `unauthorized`), go to Step 2
- Exit code `2` → config exists but is incomplete/corrupt, go to Step 2 to reconfigure/overwrite it

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

Use the stdin redirection pattern with workspace interactive prompts (no environment variables needed):

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

This will output a confirmation JSON indicating that the SMS code was sent.

**Step B — Sign in with code (Interactive):**

Execute `runInPersistentShell` (or `runInPersistentPowerShell`) with:

- `requireUserInput=true`
- `inputType=text`
- `inputPrompt=텔레그램 인증 코드를 입력하세요:`
- Command:

  ```powershell
  python "<skill-base-dir>/scripts/setup.py" --action sign_in --code-stdin
  ```

If this returns a successful auth JSON, the configuration is complete.

**Step C — Handle 2FA (if prompted):**

If Step B returns a status of `"password_needed"`, execute `runInPersistentShell` (or `runInPersistentPowerShell`) with:

- `requireUserInput=true`
- `inputType=password`
- `inputPrompt=텔레그램 2FA 비밀번호를 입력하세요:`
- Command:

  ```powershell
  python "<skill-base-dir>/scripts/setup.py" --action sign_in --password-stdin
  ```

After successful setup, proceed to Step 3 with the original user request.

---

## Step 3: Dispatch Action

Classify the user's request into one of six actions and call `telegram_cli.py`:

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

```bash
python "<skill-base-dir>/scripts/telegram_cli.py" --action search_messages \
  --query "검색어" \
  [--limit N] \
  [--chat "<chat_id_or_username>"]
```

Use when: "텔레그램에서 ~ 검색", "search telegram"

---

### Action: `download_file` — Download a file from a message

```bash
python "<skill-base-dir>/scripts/telegram_cli.py" --action download_file \
  --chat "<chat_id_or_username>" \
  --message_id N \
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

## Step 4: Present Results

- **Message list**: Show as a numbered table — `#  From  Date  Content`
- **Send confirmation**: Confirm action completed with brief summary
- **Chat list**: Show as a table — `#  Name  Type  Members/ID  Last Activity`
- **Search results**: Same as message list, with match count
- **Errors**: See Error Handling below

---

## Error Handling

| Error                | Cause                         | Action                                            |
| -------------------- | ----------------------------- | ------------------------------------------------- |
| Auth required        | Session expired or invalid    | Re-run Step 2's authentication flow               |
| Unauthorized session | `send_code` only, no sign_in  | Complete Step B/C (`sign_in`)                       |
| FloodWait            | Rate limited by Telegram      | Wait and retry after the specified seconds        |
| Chat not found       | Invalid chat ID/username      | Confirm chat identifier with user                 |
| File download failed | File size limit or permission | Check file size, try alternative download method  |
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
