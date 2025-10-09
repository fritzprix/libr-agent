# 20251009

## TO-DOs

- [x] Message에 대한 bm25 search 지원 구현
- [x] Chat History에 검색 기능 고도화, bm25 기반 메시지 검색을 통해 메시지 건수를 기준으로 session 연관성을 점수화 정렬
- [X] 사용자와 interaction을 위한 UI Builtin 도구 구현 ✅ **완료 (2025-01-09)**
  - [X] visualize_data: 데이터 시각화 (bar/line chart via SVG)
    - type: 'bar' | 'line'
    - data: Array<{ label: string; value: number }>
    - 구현 위치: `src/lib/web-mcp/modules/ui-tools.ts`
  - [X] prompt_user: 사용자 프롬프트 UI (text/select/multiselect)
    - type: 'text' | 'select' | 'multiselect'
    - prompt: string
    - options?: string[]
    - 구현 위치: `src/lib/web-mcp/modules/ui-tools.ts`
  - [X] reply_prompt: 사용자 응답 수신
    - messageId: string
    - answer: string | string[] | null
    - 구현 위치: `src/lib/web-mcp/modules/ui-tools.ts`
  - [X] Worker 등록 완료: `src/lib/web-mcp/mcp-worker.ts`
  - [X] UIAction 라우팅: 기존 `MessageRenderer.tsx`에 이미 구현되어 있음
  - [X] 전체 검증 통과: `pnpm refactor:validate` ✅
- [ ] Agent Mode 구현, AI Service의 Tool Use를 강제하도록 설정
  - ai-service의 각 서비스 provider의 구현을 검토하여 API호출 시 AI 응답에 tool use를 강제할 수 있는지 검토 필요
  - ChatContext에 agentic mode boolean state를 추가 및 관련 제어를 위한 hook 속성 추가
  - Chat UI에 Agentic Mode 토글 버튼으로 이상태를 제어하고,이에 따라 ai service가 항상 tool use를 강제하도록 (agentic mode === true) 코드 업데이트
