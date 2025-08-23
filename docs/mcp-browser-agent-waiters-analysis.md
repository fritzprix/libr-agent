# MCP 브라우저 에이전트 waiters 비어 있음 현상 분석

## 1. 현상 요약

- Rust의 `execute_script`에서 oneshot 채널을 생성하고 `result_waiters`에 `session_id`로 등록
- JS에서 결과를 만들어 `window.__TAURI_INTERNALS__.invoke('browser_script_result', { payload })` 호출
- Rust의 `handle_script_result`가 실행되지만, waiters의 keys가 비어 있음 (즉, 해당 session_id가 없음)
- Rust에서 타임아웃이 발생하기도 전에 waiters가 비어 있는 상태가 됨
- 결과적으로 JS가 결과를 보내도 Rust에서 받을 채널이 없음

## 2. 코드 흐름 (핵심 부분)

### execute_script
```rust
let (tx, rx) = oneshot::channel();
{
    let mut waiters = self.result_waiters.lock()
        .map_err(|e| format!("Failed to acquire result_waiters lock: {}", e))?;
    waiters.insert(session_id.to_string(), tx);
}
// JS 래퍼 스크립트 실행
// 결과를 기다림 (tokio::time::timeout)
```

### handle_script_result
```rust
if let Ok(mut waiters) = self.result_waiters.lock() {
    let keys: Vec<String> = waiters.keys().cloned().collect();
    info!("handle_script_result: {} / {:?} ", session_id, keys);
    if let Some(tx) = waiters.remove(session_id) {
        if tx.send(result).is_err() {
            debug!("Receiver for session {} dropped, likely due to timeout.", session_id);
        }
    } else {
        debug!("No waiting channel for session {} (likely timed out).", session_id);
    }
} else {
    return Err("Failed to acquire lock on result_waiters.".to_string());
}
```

## 3. 실제 로그 예시

```
[2025-08-21][07:11:56][tauri_mcp_agent_lib::services::interactive_browser_server][INFO] Script wrapper executed in session: 6d958f16-ada5-4b85-abae-bb52e8188972
[2025-08-21][07:11:56][tauri_mcp_agent_lib::services::interactive_browser_server][INFO] handle_script_result: 6d958f16-ada5-4b85-abae-bb52e8188972 / []
```
- JS에서 invoke가 호출되어 Rust의 handle_script_result가 실행됨
- 하지만 waiters의 keys가 비어 있음 (즉, session_id가 없음)

## 4. 원인 분석

- IPC 경로(Invoke 및 핸들러 연결)는 정상적으로 동작함
- JS와 Rust의 session_id 값이 불일치하거나, waiters에 값이 정상적으로 들어가지 않음
- 혹은, lock/스레드/비동기 환경에서 race condition이 발생해 waiters가 비는 타이밍 문제
- 중복 호출, session_id 오타, 혹은 insert/remove 시점의 불일치 가능성
- 타임라인 해설
JS에서 invoke가 호출되어 Rust의 handle_script_result가 실행됨
하지만 waiters의 keys가 이미 비어 있음 (즉, session_id가 없음)
그 뒤에야 Rust에서 timeout이 발생함
중요한 점
waiters가 비어 있는 원인은 timeout이 아님!
오히려 handle_script_result가 먼저 호출되어 waiters가 비어 있다는 로그가 남고, 그 뒤에 timeout이 발생한다는 점에서, timeout이 waiters를 비우는 직접적인 원인이 아니라는 것이 명확합니다.

## 5. 진단 포인트

- JS에서 보내는 sessionId와 Rust에서 등록하는 session_id가 완전히 동일한지 확인 필요
- waiters.insert 직후 keys를 로그로 남겨 실제로 값이 들어가는지 확인
- handle_script_result 호출 시점의 session_id와 keys를 비교해 불일치 여부 확인
- Rust에서 타임아웃이 발생하기 전에 waiters가 왜 비는지, 코드상 모든 insert/remove 경로를 추적

## 6. 결론 및 해결 방향

- IPC 자체는 정상이나, waiters에 값이 들어가지 않거나, JS와 Rust의 session_id가 불일치하는 구조적 문제
- race condition, session_id 불일치, 혹은 비동기 환경에서의 타이밍 문제를 집중적으로 추적해야 함
- 로그를 더 세밀하게 남겨서 실제로 값이 언제 들어가고 언제 사라지는지, session_id가 일치하는지 확인 필요

---

이 문서는 MCP 브라우저 에이전트의 waiters 비어 있음 현상에 대한 구조적 분석과 실제 코드/로그 예시를 포함합니다. 추가 진단 및 해결을 위해 로그와 코드의 세밀한 비교가 필요합니다.
