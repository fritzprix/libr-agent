# Output Format Guidelines

## Inbox listing

```
📬 받은 편지함 (읽지 않은 메일: 3 / 총 47개)

#  보낸 사람              제목                          날짜
1  ● 김철수 <k@corp.com>  [긴급] 서버 점검 안내         오늘 09:14
2  ● 박영희 <p@corp.com>  6월 회의 일정 공유            오늘 08:30
3    noreply@github.com   PR #142 merged                어제 23:11

● = 읽지 않음  |  더 보려면: "다음 10개 보여줘"
```

Show as numbered table: `#  From  Subject  Date  (Unread?)`

## Email body

Show full content; list attachments separately.

## Search results

Same as inbox list, with match count.

## Send/manage confirmation

```
✅ 메일 발송 완료
  받는 사람: recipient@example.com
  제목: 회의 일정 확인 요청
  발송 시각: 2026-06-01 14:23
```

## Errors

See [error-handling.md](error-handling.md).
