# Request Patterns

## Natural language → action

| User says (examples) | Action | Notes |
| --- | --- | --- |
| 이번 주 일정 / my week / weekly schedule | `week_summary` | Pass `--timezone` when known |
| 오늘 일정 / today's calendar | `list_events` | `--from` start of today, `--to` end of today |
| 내일 3시 미팅 잡아줘 | `create_event` | Confirm timezone; default 1h duration if end omitted |
| 일정 변경 / reschedule | `update_event` | Need event id — list first if unknown |
| 미팅 취소 / delete meeting | `delete_event` | Confirm before delete |
| 회의실 + 참석자 | `create_event` | `--location`, `--attendees` |

## CLI examples

Week summary (Seoul):

```bash
python scripts/calendar_cli.py --action week_summary --timezone Asia/Seoul
```

List today:

```bash
python scripts/calendar_cli.py --action list_events \
  --from 2026-06-12T00:00:00+09:00 \
  --to 2026-06-12T23:59:59+09:00
```

Create event:

```bash
python scripts/calendar_cli.py --action create_event \
  --title "Design review" \
  --start 2026-06-13T15:00:00+09:00 \
  --end 2026-06-13T16:00:00+09:00 \
  --location "Zoom" \
  --attendees "peer@example.com"
```

Delete:

```bash
python scripts/calendar_cli.py --action delete_event --event-id EVENT_ID
```
