# Request Patterns

Maps natural language requests to `email_client.py` actions.
Use this reference to determine the correct `--action` and arguments.

---

## read_inbox — Browsing the inbox

**Patterns that trigger this action:**

| User says | Command |
| --- | --- |
| "메일 보여줘" | `--action read_inbox --limit 10 --filter unread` |
| "받은 편지함" | `--action read_inbox --limit 10 --filter unread` |
| "안 읽은 메일" | `--action read_inbox --filter unread` |
| "오늘 온 메일" | `--action read_inbox --filter today` |
| "최근 메일 20개" | `--action read_inbox --limit 20 --filter all` |
| "전체 메일 목록" | `--action read_inbox --limit 20 --filter all` |
| "Check my email" | `--action read_inbox --limit 10 --filter unread` |
| "다음 10개 보여줘" | `--action read_inbox --limit 10 --filter all` (offset logic: show next page) |

**Default:** `--limit 10 --filter unread`

---

## read_email — Reading a specific email

**Patterns that trigger this action:**

| User says | Command |
| --- | --- |
| "1번 메일 열어줘" | `--action read_email --uid <uid from list item #1>` |
| "이 메일 내용 보여줘" | `--action read_email --uid <last referenced uid>` |
| "두 번째 메일 읽어줘" | `--action read_email --uid <uid from list item #2>` |
| "방금 온 메일 내용이 뭐야?" | `--action read_email --uid <uid of most recent>` |
| "Open email from 김철수" | Search first, then read the matching UID |

**Note:** Always use the UID from a prior `read_inbox` or `search_email` response.
Never guess UIDs. If the user refers to an email by number, map to the `uid` field
from the numbered list shown to the user.

---

## send_email — Composing and sending

**Patterns that trigger this action:**

| User says | Approach |
| --- | --- |
| "김철수한테 메일 보내줘" | Ask for subject/body if not provided, then send |
| "이 메일에 답장 써줘" | Use `--reply-to-uid`, prepend "Re:" to subject |
| "회의 일정 잡는 메일 써줘" | Draft full body based on context, confirm before sending |
| "박 팀장한테 보고서 보냈다고 해줘" | Draft short message, confirm recipient address, send |
| "Forward this to HR" | Read original, compose new message with original body quoted |

**Draft confirmation rule:**
If the user does not provide a full body, draft the email content yourself,
show it to the user for approval ("이 내용으로 발송할까요?"), then send after confirmation.

**Example composed command:**
```
python email_client.py --action send_email \
  --to "chulsoo.kim@corp.com" \
  --subject "6월 정기 회의 일정 확인" \
  --body "안녕하세요, 김철수님.\n\n6월 정기 회의 일정을 확인 부탁드립니다.\n..."
```

---

## search_email — Searching

**Patterns that trigger this action:**

| User says | IMAP query |
| --- | --- |
| "김철수한테 온 거 찾아줘" | `FROM "김철수"` |
| "boss@corp.com에서 온 메일" | `FROM "boss@corp.com"` |
| "프로젝트 관련 메일" | `SUBJECT "프로젝트"` |
| "지난주 메일" | `SINCE 26-May-2026` |
| "5월에 온 메일" | `SINCE 01-May-2026 BEFORE 01-Jun-2026` |
| "안 읽은 메일 중 긴급" | `UNSEEN SUBJECT "긴급"` |
| "첨부파일 있는 메일" | `BODY "Content-Disposition: attachment"` (best effort) |
| "계약서 관련" | `TEXT "계약서"` |

**IMAP search operator reference:**

| Operator | Meaning |
| --- | --- |
| `FROM "x"` | Sender contains x |
| `TO "x"` | Recipient contains x |
| `SUBJECT "x"` | Subject contains x |
| `TEXT "x"` | Body or header contains x |
| `UNSEEN` | Unread messages |
| `SEEN` | Read messages |
| `SINCE dd-Mon-yyyy` | On or after date |
| `BEFORE dd-Mon-yyyy` | Before date |
| `ON dd-Mon-yyyy` | Exactly on date |
| `FLAGGED` | Starred / flagged |

Combine operators with a space (implicit AND):
`FROM "boss" SINCE 01-May-2026 UNSEEN`

---

## manage_email — Managing emails

**Patterns that trigger this action:**

| User says | Command |
| --- | --- |
| "읽음 처리해줘" | `--action manage_email --uid <uid> --op mark_read` |
| "안 읽음으로 바꿔줘" | `--action manage_email --uid <uid> --op mark_unread` |
| "이 메일 삭제해줘" | `--action manage_email --uid <uid> --op delete` |
| "스팸함으로 이동" | `--action manage_email --uid <uid> --op move --folder Spam` |
| "보관함으로 이동" | `--action manage_email --uid <uid> --op move --folder Archive` |
| "중요 폴더로" | `--action manage_email --uid <uid> --op move --folder Important` |

**Bulk operations (e.g. "스팸함 비워"):**
1. Search the target folder: `--action search_email --query "ALL"`
2. Show count to user: "스팸함에 37개 있습니다. 모두 삭제할까요?"
3. After confirmation, loop through UIDs with `--op delete`

**Common folder names** (vary by server):

| Folder type | Gmail | Outlook | Naver |
| --- | --- | --- | --- |
| Spam | `[Gmail]/Spam` | `Junk Email` | `스팸메일함` |
| Archive | `[Gmail]/All Mail` | `Archive` | `보관함` |
| Sent | `[Gmail]/Sent Mail` | `Sent Items` | `보낸메일함` |
| Trash | `[Gmail]/Trash` | `Deleted Items` | `휴지통` |

If folder move fails, try listing folders first (IMAP `LIST "" "*"` — not exposed as a
standalone action, but can be added if needed).

---

## Ambiguous requests — how to resolve

| Situation | Resolution |
| --- | --- |
| User says "메일 확인해줘" with no context | Default to `read_inbox --filter unread --limit 10` |
| User refers to "그 메일" without specifying | Ask: "어떤 메일을 말씀하시는 건가요? 목록에서 번호를 알려주세요." |
| User asks to "forward" | Read original → compose new with quoted body → confirm → send |
| User wants to reply but gives no body | Draft reply based on original content and context, confirm before send |
| User asks "몇 개나 왔어?" | Run `read_inbox --filter all` and report `total` + `unread` counts only |
