# 📋 개발팀 작업 지시 — Claude/Channel MCP 통합

**작성일:** 2026-06-27 01:00 KST  
**지시자:** Coding Expert  
**우선순위:** P0 (프롬프트 주입 완화) / P1 (기타 개선)

---

## 📄 참조 문서 (반드시 먼저 읽을 것)

| 문서                     | 경로                                                                                       | 용도                          |
| ------------------------ | ------------------------------------------------------------------------------------------ | ----------------------------- |
| **전체 레퍼런스 가이드** | `//?/C:/Users/innoc/my_works/libr-agent/docs/mcp/claude-channels-mcp-server-reference.md`  | 프로토콜 스펙, 예제 코드, API |
| **개발팀 공지**          | `//?/C:/Users/innoc/my_works/libr-agent/docs/mcp/claude-channels-dev-team-announcement.md` | 핵심 스펙 요약, 구현 상태     |

---

## 🎯 작업 항목

### P0 — 프롬프트 주입 완화 (최우선)

**파일:** `//?/C:/Users/innoc/my_works/libr-agent/src-tauri/src/agent/session_manager/channel.rs`

**문제:** 채널 메시지가 `role: "user"`로 주입될 때 내용 필터링이 전혀 없음. 악성 MCP 서버가 임의 지시를 삽입 가능.

**작업:**

1. `build_channel_message()` 함수에 콘텐츠 필터링/분류 로직 추가
2. 위험 키워드/패턴 감지 시 경고 로그 또는 거부 처리
3. 선택: 메시지 앞에 시스템 프롬프트 경계 주석 주입 (`<!-- CHANNEL_MESSAGE_START -->`)

**테스트 파일:** `//?/C:/Users/innoc/my_works/libr-agent/src-tauri/tests/` (새 통합 테스트 추가)

---

### P1 — 속성 필터 개선

**파일:** `//?/C:/Users/innoc/my_works/libr-agent/src-tauri/src/agent/session_manager/channel.rs`

**작업 1: `data-*` 속성 차단**

- `is_safe_channel_attribute_name()` 함수에 `data-*` 접두사 차단 추가
- 프론트엔드 HTML 렌더링 추가 시 잠재적 XSS 위험 제거

**작업 2: `[channel_meta]` 본문 이스케이프**

- `format_channel_payload()`에서 `[channel_meta]` 블록의 키/값에 `[` `]` 이스케이프 적용
- 파서 혼란 방지

---

### P1 — 드롭 메트릭스 테스트 추가

**파일:** `//?/C:/Users/innoc/my_works/libr-agent/src-tauri/src/mcp/session_isolation/channel_events.rs`

**작업:** `maybe_log_channel_drop_metrics()`의 60초 throttling 로직에 대한 테스트 추가

- 60초 이내 재호출 시 로그 미출력 검증
- 60초 경과 시 로그 출력 + 카운터 리셋 검증

**테스트 위치:** `//?/C:/Users/innoc/my_works/libr-agent/src-tauri/tests/channel_drop_metrics_throttle_tests.rs`

---

### P2 — Native stdio path 완성

**파일:** `//?/C:/Users/innoc/my_works/libr-agent/src-tauri/src/mcp/session_isolation/channel_transport.rs`

**작업:** `notifications/claude/channel` 직접 수신이 bridge(HTTP/Tauri)에 이어 추가되도록 transport 확장

---

## 🔧 검증 절차

모든 수정 후 다음 명령 실행 필수:

```bash
cd //?/C:/Users/innoc/my_works/libr-agent
pnpm refactor:validate
```

이 명령은 lint, format, Rust 빌드, dead-code 체크를 모두 포함합니다.

---

## 📅 마감

| 작업                 | 우선순위 | 권장 마감  |
| -------------------- | -------- | ---------- |
| 프롬프트 주입 완화   | P0       | 2026-07-04 |
| 속성 필터 개선       | P1       | 2026-07-04 |
| 드롭 메트릭스 테스트 | P1       | 2026-07-04 |
| Native stdio path    | P2       | 2026-07-11 |

---

_문의사항은 이ス레드에서 답변. PR 생성 시 이 파일을 첨부된 참고 문서로 명시._
