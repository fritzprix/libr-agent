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

## Dependencies

Required dependencies (should already be installed):

- **telethon**: `pip3 install telethon` (MTProto client library)

## References

These reference files are located in the skill directory:
- [cli-reference.md](file:///home/fritzprix/my_works/libr-agent/src-tauri/bundled_skills/telegram-cli/references/cli-reference.md) — CLI commands, options, outputs, and errors reference

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

Execute `workspace__runInPersistentShell` (or `workspace__runInPersistentPowerShell`) with:

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

### Input encoding (Windows / Unicode workaround)

On Windows, passing Unicode characters (like Korean) as command-line arguments can cause character corruption (mojibake) due to PowerShell's default encoding (`cp949`).

To bypass this bottleneck, **always** write the message or query to a UTF-8 file and use `--message-file` or `--query-file` instead of `--message` or `--query`:

#### Send Message (Windows Recommended)
```powershell
# 1. Write the message to a UTF-8 file in the workspace
$msg = "안녕하세요, 텔레그램 메시지 테스트입니다."
$msg | Out-File -Encoding utf8 "<workspace>/tg_message.txt"

# 2. Call the CLI with --message-file
python "<skill-base-dir>/scripts/telegram_cli.py" --action send_message `
  --chat "<chat_id_or_username>" `
  --message-file "<workspace>/tg_message.txt"
```

#### Search Messages (Windows Recommended)
```powershell
# 1. Write the query to a UTF-8 file in the workspace
$query = "테스트"
$query | Out-File -Encoding utf8 "<workspace>/tg_query.txt"

# 2. Call the CLI with --query-file
python "<skill-base-dir>/scripts/telegram_cli.py" --action search_messages `
  --query-file "<workspace>/tg_query.txt" `
  --output "<workspace>/telegram_search.json"
```

### Dispatch CLI Actions

Classify the user's request into one of the actions (`send_message`, `get_messages`, `list_chats`, `search_messages`, `download_file`, `get_chat_info`) and execute the CLI. For a detailed reference of CLI parameters, JSON output schemas, and pagination strategies, see the [cli-reference.md](references/cli-reference.md) guide.

---

## Step 4: Present Results

- **Message list**: Show as a numbered table — `#  From  Date  Content`
- **Send confirmation**: Confirm action completed with brief summary
- **Chat list**: Show as a table — `#  Name  Type  Members/ID  Last Activity`
- **Search results**: Same as message list, with match count
- **Errors**: See Error Handling in [cli-reference.md](references/cli-reference.md)

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
