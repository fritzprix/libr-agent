---
title: 에러 코드 · 해결
---

# 에러 코드 및 해결 가이드

메시지가 조금 달라도 **증상 → 확인 위치**만 맞으면 됩니다. UI 기준은 사이드바 **Settings** / **Extensions** / **History** / **Chat**입니다.

---

## API · 인증

### `API key is invalid` / `API key is missing`

**확인:** 사이드바 **Settings** → **AI & Models** → **Provider API Keys** → 키 재입력 → **Save Changes**

키는 제공사 콘솔에서 재발급하세요 (Anthropic / OpenAI / Google AI Studio 등).

### `Rate limit exceeded`

잠시 대기 후 재시도. **Default LLM** 또는 세션 Model을 다른 모델/Provider로 바꾸거나, 동시 세션·예약 작업을 줄입니다.

### `401` / `Authentication failed`

키 앞뒤 공백 없이 다시 붙여넣기. MCP라면 **Extensions**에서 해당 서버의 env/인증 필드를 다시 확인합니다.

---

## 연결 · 네트워크

### `Connection refused`

Ollama·로컬 MCP 프로세스가 떠 있는지, Base URL/포트가 맞는지 확인합니다.  
MCP: **Extensions**에서 command·args·cwd를 점검하고, 터미널에서 같은 명령을 직접 실행해 봅니다. → [커스텀 MCP](../guides/custom-mcp.md)

### `Timeout` / `Request timed out`

작업 범위를 줄이거나, 네트워크·MCP 응답을 확인한 뒤 재시도합니다. 장시간 하위 에이전트는 [서브 에이전트](../guides/sub-agents.md) 흐름으로 나눕니다.

### MCP가 응답 없음

**Extensions**에서 서버 설정 확인 → 의존성(`npx`/`uv` 등) → 제거 후 다시 추가. → [Extensions](../guides/extensions.md)

---

## 세션 · 모델 · 도구

### `Session not found`

**History**에서 세션이 있는지 확인. 삭제됐다면 **Chat**에서 새 세션을 시작합니다.

### `Model not found` / `Model unavailable`

**Settings → AI & Models**의 **Default LLM**, 또는 Chat의 Model 피커에서 **Refresh models** 후 사용 가능한 모델을 고릅니다. Assistants에 묶인 기본 모델도 확인합니다.

### `Tool execution failed`

응답의 도구 오류 내용을 에이전트에게 붙여 “원인 확인 후 수정해줘”라고 요청합니다. 워크스페이스 경로·OS 권한·YOLO/승인 대기 상태도 봅니다.

### `Context window exceeded`

새 세션을 열고, 필요하면 이전 요약을 짧게 붙여 이어갑니다. 한 세션에 큰 파일·긴 로그를 과다 첨부하지 마세요.

---

## 파일

### `Permission denied` / `File not found`

세션 워크스페이스 경로와 OS 권한을 확인합니다. 하위 에이전트는 부모 폴더를 **자동 상속하지 않습니다** — [서브 에이전트](../guides/sub-agents.md).

---

## 관련

- [문제 해결](../guides/troubleshooting.md)
- [모델 연결](../getting-started/connecting-models.md)
- [FAQ](./common-questions.md)

지원: [Discussions](https://github.com/fritzprix/libr-agent/discussions) — **API 키 제외**.
