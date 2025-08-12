# Refactoring Plan

> Built-In Tool 지원을 위한 체계적 구조 필요

## 문제점

- 현재 WebMCPContext는 Web Worker를 통한 Built-In 도구 통합을 지원
- 하지만 현재는 단순히 Tools만 추가될 수 있는 구조임
  - UnifiedMCPContext를 통해 Tauri를 통해 rmcp.rs를 통해 연결된 MCP 서버와 묶여서 전달이 됨

## 원하는 것

- Built-In Tools의 제공 여부에 대한 Control 기능 제공
- 또한 Tools와 함께 추가적인 상태 혹은 정보를 LLM API의 Context Window에 추가할 수 있는 구조가 필요

## 접근 방향

- AssistantExtensionContext.tsx를 활용
- 대략 아래와 같은 패턴으로 명시적으로 이 WebMCP의 기능을 AssistantExtensionContext.tsx를 활용하여 적절한 SystemPrompt와 함께 확장하고자 함
- 참고 ./src/app/BuiltIn.tsx (대략적인 상호작용만 Draft)