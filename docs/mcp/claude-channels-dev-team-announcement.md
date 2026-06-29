# 📢 LibrAgent Claude/Channel — MCP Server 개발자 레퍼런스 가이드 출시

**발행일:** 2026-06-27  
**작성자:** Coding Expert  
**참조 문서:** `//?/C:/Users/innoc/my_works/libr-agent/docs/mcp/claude-channels-mcp-server-reference.md`

---

## 🎯 개요

LibrAgent의 `claude/channel` 프로토콜과 호환되는 **외부 MCP 서버**를 개발하기 위한 완전한 레퍼런스 가이드가 작성되었습니다.

이 문서는 `//?/C:/Users/innoc/my_works/libr-agent/src-tauri/src/` 백엔드 구현의 소스 코드를 직접 분석하여 작성했으며, 외부 개발자가 자체 채널 서버를 구축·연동하는 데 필요한 모든 기술 사양을 포함합니다.

---

## 📄 레퍼런스 가이드 구조

| 섹션                                   | 내용                                                           |
| -------------------------------------- | -------------------------------------------------------------- |
| **1. Overview**                        | Bridge (HTTP/Tauri) vs Native stdio 아키텍처 비교              |
| **2. Server Capability Advertisement** | `initialize` 응답에 `experimental['claude/channel']` 포함 방법 |
| **3. Outbound Messages**               | MCP 서버 → LibrAgent: JSON-RPC stdout 전송 형식                |
| **4. Inbound Messages**                | LibrAgent → Agent: XML 페이로드 포맷 및 안전 필터링            |
| **5. Permission Relay**                | 도구 승인 요청/응답 프로토콜 (`claude/channel/permission`)     |
| **6. Auto-Routing**                    | 세션 라우팅 로직 (1-match → inject, 0/2+ → error)              |
| **7. Integration Endpoints**           | Tauri 명령어 + HTTP API 전체 목록                              |
| **8. Drop Metrics**                    | 버퍼 오버플로우/리스너 종료 모니터링                           |
| **9. Example Implementations**         | Python, Node.js, Raw stdio 예제 코드                           |
| **10. Security**                       | 속성 필터링, XML 이스케이프, 콘텐츠 크기 제한, 프롬프트 주입   |
| **11. Implementation Status**          | 구현 완료/진행 중/미구현 기능 매트릭스                         |
| **12. Troubleshooting**                | 증상 → 원인 → 해결 매핑                                        |

**전체 분량:** ~630 라인, 약 25 KB

---

## 🔑 핵심 기술 스펙 요약

### 1. 서버 능력 선언 (initialize 응답)

MCP 서버는 `initialize` 응답의 `experimental` 필드에 다음 capabilities를 포함해야 합니다:

```json
{
  "experimental": {
    "claude/channel": {},
    "claude/channel/permission": {}
  }
}
```

| Capability                  | 용도                             |
| --------------------------- | -------------------------------- |
| `claude/channel`            | 에이전트 세션에 메시지 푸시 지원 |
| `claude/channel/permission` | 도구 승인 릴레이 지원 (선택사항) |

### 2. 메시지 전송 형식 (stdout)

```json
{
  "jsonrpc": "2.0",
  "method": "claude/channel",
  "params": { "content": "hello", "meta": { "chat_id": "12345" } }
}
```

| 항목            | 사양                                                           |
| --------------- | -------------------------------------------------------------- |
| **전송**        | JSON-RPC 2.0 notification, stdout 한 줄당 하나                 |
| **인코딩**      | UTF-8, `\n` 라인 종료                                          |
| **콘텐츠 제한** | 최대 8,192 bytes (초과 시 silent drop + warn 로그)             |
| **버퍼**        | 세션당 1,024 이벤트 (오버플로우 시 drop + 60초 간격 warn 로그) |

### 3. XML 페이로드 변환

LibrAgent가 수신한 메시지를 에이전트 대화에 주입할 때 XML로 변환:

```xml
<channel source="telegram" chat_id="12345" sender_name="Alice">
Hello from the channel!

[channel_meta]
sender_id=42
[/channel_meta]
</channel>
```

| 필터링 항목        | 동작                                                             |
| ------------------ | ---------------------------------------------------------------- |
| `source`           | 항상 `server_name`으로 예약 → 차단 (case-insensitive)            |
| `style`            | XSS 위험 → 차단 (case-insensitive)                               |
| HTML 이벤트 핸들러 | `onclick`, `onerror`, `onload` 등 **명시적 화이트리스트**로 차단 |
| 안전하지 않은 키   | `[channel_meta]` body 블록으로 폴백 (silent drop 아님)           |
| XML 이스케이프     | `&`→`&amp;`, `"`→`&quot;`, `<`→`&lt;`, `>`→`&gt;`                |

### 4. 승인 릴레이 프로토콜

```
LibrAgent → MCP 서버: notifications/claude/channel/permission_request
MCP 서버 → LibrAgent: claude/channel/permission (verdict: "allow" \| "deny")
```

| 필드         | 사양                                                   |
| ------------ | ------------------------------------------------------ |
| `request_id` | UUID v4, 32-char lowercase hex (예: `a1b2c3d4e5f6...`) |
| `behavior`   | `"allow"` 또는 `"deny"`만 허용                         |

### 5. Auto-Routing

| 매칭 세션 수 | 동작                                                                   |
| ------------ | ---------------------------------------------------------------------- |
| 0            | `No active session...` 에러                                            |
| 1            | 해당 세션에 메시지 주입                                                |
| 2+           | `Ambiguous active sessions...` 에러 (세션 스코프 엔드포인트 사용 필수) |

---

## 🚧 현재 구현 상태

| 기능                         | 상태                |
| ---------------------------- | ------------------- |
| 서버 능력 발견               | ✅                  |
| 채널 메시지 포맷팅           | ✅                  |
| Bridge: 세션리스 진입        | ✅                  |
| Bridge: 세션스코프 진입      | ✅                  |
| Bridge: 승인 릴레이          | ✅                  |
| 프론트엔드 채널 렌더링       | ✅                  |
| Native stdio transport       | ✅                  |
| Native `claude/channel` 파싱 | ✅                  |
| Auto-routing                 | ✅                  |
| 드롭 메트릭스 로깅           | ✅                  |
| 프롬프트 주입 완화           | ❌ (별도 수정 필요) |

---

## ⚠️ 알려진 제한사항

1. **프롬프트 주입 취약점** — 채널 메시지는 `role: "user"`로 주입되며 내용 필터링이 없습니다. 악성 MCP 서버가 임의 지시를 삽입할 수 있습니다. 별도 보안 수정이 필요합니다.
2. **`data-*` 속성 통과** — `data-*` 접두사 속성이 필터를 통과합니다. 프론트엔드 HTML 렌더링 추가 시 잠재적 위험.
3. **`[channel_meta]` 본문 주입** — 안전하지 않은 메타 키가 본문에 기록될 때 `[channel_meta]`/`[/channel_meta]` 시퀀스가 파서를 혼란스럽게 할 수 있습니다.

---

## 📍 참조 파일

| 파일                                                                                              | 설명                         |
| ------------------------------------------------------------------------------------------------- | ---------------------------- |
| `//?/C:/Users/innoc/my_works/libr-agent/docs/mcp/claude-channels-mcp-server-reference.md`         | **전체 레퍼런스 가이드**     |
| `//?/C:/Users/innoc/my_works/libr-agent/src-tauri/src/agent/session_manager/channel.rs`           | XML 포맷팅, 속성 필터링      |
| `//?/C:/Users/innoc/my_works/libr-agent/src-tauri/src/mcp/session_isolation/channel_events.rs`    | JSON-RPC 파싱, 드롭 메트릭스 |
| `//?/C:/Users/innoc/my_works/libr-agent/src-tauri/src/mcp/session_isolation/channel_transport.rs` | stdio transport              |
| `//?/C:/Users/innoc/my_works/libr-agent/src-tauri/src/agent/channel_routing.rs`                   | 세션 라우팅                  |
| `//?/C:/Users/innoc/my_works/libr-agent/src-tauri/src/mcp/types.rs`                               | 데이터 구조체                |

---

## 🎯 다음 단계

1. **레퍼런스 가이드 공유** — MCP 서버 개발팀에 배포 및 리뷰 요청
2. **프롬프트 주입 완화 (P0)** — 채널 메시지 콘텐츠 필터링/분류 구현
3. **`data-*` 속성 차단 (P2)** — 속성 필터에 `data-*` 접두사 추가
4. **Native path 완성** — `notifications/claude/channel*` 직접 수신이 bridge에 이어 추가 예정

---

_이 문서는 `//?/C:/Users/innoc/my_works/libr-agent/src-tauri/src/`의 실제 구현 코드를 직접 분석하여 작성되었습니다. 구현이 변경될 때마다 문서도 함께 업데이트해야 합니다._
