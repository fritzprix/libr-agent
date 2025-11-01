# Chat UI/UX 개선

- stream mode API를 사용하고 있음에도 답답한 느낌
- streaming 중 message bubble이 업데이트되어야 함
- `thinking ...` 단조로운 loading ui가 지루함을 더 크게 느끼게 함
- 사용자의 message는 간결한 bubble로 표시
- Agent의 답변은 보다 Rich하게 Bubble이 아닌 Message Window에 Embedded 된 형태로 표시 (ChatGPT 등과 유사)
- 보다 풍부한 markdown Rendering / Coding Block에 적절한 Syntax Highlight 지원
- tool call 및 tool response는 간결히 표시
- tool response 중 UI Resource는 bubble이 아니라 Message Window에 Embedded 된 형태로 표시
