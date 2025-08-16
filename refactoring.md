# Refactoring Plan — 최근 ChatContext.tsx 변경과 관련 ChatInput 개선

- 최근 코드 변경 사항
  - ChatContext와 cancel 기능이 추가됨

- 현재 문제점
  - ChatInput에서 이러한 cancel을 지원하기 위해
  - 기존 submit 버튼이 ChatContext의 isLoading에따라 아래와 같이 되어야 함
    - isLoading이 true일 때 cancel button으로 되어야 함
    - isLoading이 false일 때 send button으로 되어야 함
- 추가 개선점
  - 첨부를 기존 버튼에서 Drag and Drop으로 변경할 것
  - Tauri v2에서 drag and drop 구현 관련 ./docs/tauri/dragdrop.md 참고
