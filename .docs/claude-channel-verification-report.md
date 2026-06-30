# 📋 claude/channel 구현 검증 결과 — 개발팀 전달용

**검증 일시**: 2026-06-26  
**검증 방식**: 3개 렌즈 컨센서스 데리게이션 (Operability / Correctness / Security)  
**종합 판정**: ⚠️ PARTIAL — 핵심 흐름은 작동하지만 P0/P1 패치가 필요함

---

## ✅ 현재 잘 작동하는 부분 (Happy Path)

MCP 서버가 `claude/channel` notification을 push하면:

1. **파싱**: `normalize_channel_method()`가 `claude/channel`, `notifications/claude/channel`, `/permission` 변형 전부 처리
2. **라우팅**: `spawn_session_channel_dispatch_task()`가 이벤트를 `inject_channel_notification` / `respond_channel_permission`로 분기
3. **인젝션**: `build_channel_message()`가 `role: "user"`, `source: MessageSource::Channel` 메시지 생성
4. **워크플로우 트리거**: idle 세션 → Queued → Busy → `request_llm_completion_with_recovery()` 호출
5. **권한 해결**: `respond_channel_permission()`이 `request_id` → pending approval → oneshot 채널로 결과 전달

**결론**: MCP 서버 → 세션 인젝션 → LLM 응답 재개 흐름은 구현되어 있음.

---

## 🔴 P0 — 반드시 고쳐야 함 (Production Blocker)

### Bug 1: 잘못된 permission verdict → 세션 영구 정지

- **파일**: `src-tauri/src/mcp/session_isolation/channel_dispatch.rs:54-62`
- **증상**: LLM이 `"maybe"`, `"allowx"` 같은 유효하지 않은 verdict를 보내면 `continue`로 `respond_channel_permission()`을 건너뛴다. pending approval이 resolve되지 않고 세션이 Busy 상태로 영구 정지
- **원인**: `parse_channel_permission_behavior()`가 `"allow"`/`"deny"` 외 값에 `Err` 반환 → dispatch task가 error 로그만 찍고 `continue`
- **수정**: invalid verdict 시 `deny` (false)로 default 처리

```rust
// Before (current)
Err(error) => {
    error!("Invalid channel permission verdict from '{}': {}", server_name, error);
    continue;  // ← BUG: pending approval이 unresolved 상태로 남음
}

// After (fix)
Err(error) => {
    warn!("Invalid channel permission verdict from '{}' (defaulting to deny): {}", server_name, error);
    approved = false;  // deny로 resolve → pending approval 정리됨
}
```

### Bug 2: 콘텐츠 크기 제한 없음 → 메모리/컨텍스트 오버플로우

- **파일**: `src-tauri/src/mcp/session_isolation/channel_events.rs:98`
- **증상**: 악의적 MCP 서버가 10MB+ 문자열을 push하면 메모리 소모 + LLM 컨텍스트 오염
- **수정**: `try_parse_channel_event()`에서 `content` 길이 검증 (권장: 8KB)

```rust
// try_parse_channel_event()의 content 추출 부분에 추가
let content = params.get("content")?.as_str()?;
if content.len() > 8192 {
    warn!("Channel message content too large ({} bytes) from '{}'; dropping", content.len(), server_name);
    return None;
}
```

---

## 🟠 P1 — 가능하면 수정 (Should Fix)

### Issue 1: 콘텐츠 필터링 부재

- **파일**: `src-tauri/src/agent/session_manager/channel.rs:build_channel_message()`
- **내용**: channel 메시지는 `role: "user"`로 주입되지만 사용자 입력 sanitization pipeline을 우회함
- **권장**: `MessageSource::Channel` 메시지에 대한 별도 필터링 또는 경고 메커니즘 도입

### Issue 2: permission request ID 추측 가능

- **파일**: `src-tauri/src/agent/tool_approvals.rs:245-254`
- **내용**: 5자 × 35자 alphabet → ~26.2 bits entropy (35^5 ≈ 52M). 같은 머신의 악의적 MCP 서버가 brute-force 가능
- **권장**: 32자 이상으로 증가 + UUIDv4 기반

### Issue 3: 채널 이벤트 rate limiting 부재

- **파일**: `src-tauri/src/mcp/session_isolation/channel_events.rs:41-55`
- **내용**: bounded channel (1024)가 full이면 이벤트가 silently drop. 악의적 서버가 연속 retry로 CPU 소비
- **권장**: sliding window rate limiter 또는 exponential backoff

---

## 🟡 P2 — 개선 사항 (Nice to Have)

| 우선순위 | 항목                     | 파일                        | 설명                                                           |
| -------- | ------------------------ | --------------------------- | -------------------------------------------------------------- |
| P2       | busy 세션 메시지 큐 알림 | `message_injection.rs:8-27` | 세션이 busy일 때 channel 메시지 유입을 UI에 알림               |
| P2       | meta attribute 네임 제한 | `channel.rs:153-161`        | `onerror`, `onclick`, `style` 등 dangerous attribute name 차단 |
| P2       | dropped event 로깅       | `channel_events.rs:41-55`   | buffer overflow 시 dropped count 카운터 + periodic log         |

---

## 📁 관련 소스 파일 인덱스

| 파일                                                       | 역할                                                     |
| ---------------------------------------------------------- | -------------------------------------------------------- |
| `src-tauri/src/mcp/session_isolation/channel_events.rs`    | `claude/channel` 파싱, 버퍼, `try_parse_channel_event()` |
| `src-tauri/src/mcp/session_isolation/channel_dispatch.rs`  | 이벤트 라우팅, dispatch task, permission 처리            |
| `src-tauri/src/agent/session_manager/channel.rs`           | 메시지 빌드, XML 포매팅, `inject_channel_notification()` |
| `src-tauri/src/agent/session_manager/message_injection.rs` | idle 감지, 상태 전환, 워크플로우 트리거                  |
| `src-tauri/src/agent/session_manager/approvals.rs`         | pending approval resolve, `respond_channel_permission()` |
| `src-tauri/src/agent/tool_approvals.rs`                    | permission behavior 파싱, request_id 생성/매칭           |

---

## 🎯 요약

| 항목                                 | 상태         |
| ------------------------------------ | ------------ |
| 핵심 흐름 (push → inject → workflow) | ✅ 작동      |
| P0 버그 2개                          | 🔴 패치 필요 |
| P1 보안/검증 3개                     | 🟠 권장      |
| P2 개선 3개                          | 🟡 선택      |

**P0만 고치면 production-ready.** P1은 untrusted MCP server 사용 시 반드시 필요.
