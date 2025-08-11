# Refactoring Plan

## As-Is (문제점)

- LocalToolContext.tsx는 실제 사용되고 있지 않는것으로 추정
- Context는 제공되고 있으나 실제 LocalService를 구현하여 공급하고 있는 것이 존재하지 않음

## To-Be (해결)

- LocalToolContext.tsx 삭제 및 다음과 같은 파일에서 관련 코드를 제거

- used in the following files:
  - App.tsx (as LocalToolProvider)
  - AssistantExtensionContext.tsx (as LocalService)
  - LocalServicesEditor.tsx (as useLocalTools)
  - Chat.tsx (as useLocalTools)
  - ToolCaller.tsx (as useLocalTools)
  - ToolsModal.tsx (as useLocalTools)
  - use-ai-service.ts (as useLocalTools)
  - GoogleDriveService.tsx (as types and/or hooks from LocalToolContext)
