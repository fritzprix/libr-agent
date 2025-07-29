# Refactoring Plan

## Assigned: @Copilot

## 문제 정의

- 문제 정의
  - ./docs/mcp.md에 정의된 tool 정의 규격이 현재 code에서 사용하고 있는 MCPTool과 다름
  - MCPTool을 이에 맞추는 것이 향후 확장성 관점에서 유리

## Task 1 - `MCPTool` 타입의 수정

- ./docs/mcp.md에 정의된 Schema에 맞도록 interface를 수정
- 이와 관련된 Code들 MCPTool를 import하고 있는 코드들을 이에 맞게 수정

### 수정이 필요한 파일 목록

- src/lib/tauri-mcp-client.ts
- src-tauri/src/mcp.rs
- src-tauri/src/lib.rs
- src/models/chat.ts
- src/context/MCPServerContext.tsx
- src/lib/ai-service.ts
- src/context/LocalToolContext.tsx
