# Telegram CLI Command & Usage Reference

This guide provides details on Telegram CLI actions, schemas, exit codes, and runtime error handling.

---

## 1. Telegram CLI Actions

Use `telegram_cli.py` to dispatch actions.

### Action: `send_message` — Send a message
```powershell
python "<skill-base-dir>/scripts/telegram_cli.py" --action send_message `
  --chat "<chat_id_or_username>" `
  --message "메시지 내용" `
  [--file "/path/to/attachment"]
```
- Username: `@username` or `username`
- Chat ID: `-1001234567890` (group/channel) or `123456789` (user)
- `me` for self (saved messages)

### Action: `get_messages` — Read recent messages
```powershell
python "<skill-base-dir>/scripts/telegram_cli.py" --action get_messages `
  --chat "<chat_id_or_username>" `
  [--limit N] `
  [--offset_id N]
```

### Action: `list_chats` — List all chats/channels/groups
```bash
python "<skill-base-dir>/scripts/telegram_cli.py" --action list_chats
```

### Action: `search_messages` — Search messages
```powershell
python "<skill-base-dir>/scripts/telegram_cli.py" --action search_messages `
  --query "검색어" `
  [--limit N] `
  [--chat "<chat_id_or_username>"] `
  [--offset_id N] `
  [--output "<workspace>/telegram_search.json"]
```

### Action: `download_file` — Download a file from a message
```powershell
python "<skill-base-dir>/scripts/telegram_cli.py" --action download_file `
  --chat "<chat_id_or_username>" `
  --message_id N `
  [--dest "/path/to/destination"]
```

### Action: `get_chat_info` — Get chat/channel info
```bash
python "<skill-base-dir>/scripts/telegram_cli.py" --action get_chat_info \
  --chat "<chat_id_or_username>"
```

---

## 2. AI Agent Usage Guide

### Standard Workflow
1. Run `check_config.py` — stop and re-auth if not `"status": "ok"`.
2. If the user gave a chat **name** (not `@username` / numeric ID), run `list_chats` first and pick the matching `id` or `username`.
3. Run the target action with `--output <workspace>/telegram_<action>.json` for message/search/list payloads.
4. Read the JSON file (not raw stdout) and format results for the user.
5. If the user asks for more, paginate with `next_offset_id`.

**Chat ID resolution order:** `list_chats` → match by `name` / `username` → use numeric `id` or `@username` in `--chat` → `get_chat_info` if metadata is needed.

### Usage examples
Replace `<skill-base-dir>` with this skill's absolute path and `<workspace>` with the agent session workspace.

**List chats:**
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

### `--output` behavior

| Mode | stdout | Full payload |
| ---- | ------ | ------------ |
| Without `--output` | Full JSON | stdout |
| With `--output path` | Compact summary JSON | UTF-8 file at `path` |

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

---

## 3. Exit codes & Pagination

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

---

## 4. Error Handling

### Error handling (agent actions)

| Error / pattern | Detection | What the agent should do |
| --------------- | --------- | ------------------------ |
| Chat not found | `"message": "Chat not found: ..."` (exit `2`) | Run `list_chats --output chats.json`, find correct `id`/`username`, retry |
| FloodWait | `"message": "FloodWait: wait N seconds"` (exit `2`) | Wait **N** seconds, retry same command |
| Not authorized | `"Not authorized. Run setup first."` (exit `1`) | Run `check_config.py`; if not ok, run Step 2 auth flow |
| Auth restart | `check_config` → `auth_restart_needed` | Delete `~/.libragent/telegram_session.session`, `send_code` → `sign_in` |
| Missing `--chat` / `--query` | exit `3` | Add required flag and retry |
| cp949 / garbled stdout | Mojibake in console | Re-run with `--output` and read the UTF-8 file |

### Error Handling (setup & auth)

| Error | Cause | Action |
| --- | --- | --- |
| Auth required | Session expired or invalid | Re-run Step 2's authentication flow |
| `auth_restart_needed` | Telegram auth state corrupted | Delete session file, run `send_code` again |
| Unauthorized session | `send_code` only, no sign_in | Complete Step B/C (`sign_in`) |
| ambiguous `--code` | Old docs used `--code` | Use `--code-value`, `--code-env`, or `--code-stdin` |
| API hash invalid | Wrong or revoked API hash | Re-run Step 2 with correct credentials |
| Phone number invalid | Format error | Guide user to enter international format (+82...) |
