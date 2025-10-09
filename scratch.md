# 20251009

## TO-DOs

- [ ] Message에 대한 bm25 search 지원 구현
- [ ] Chat History에 검색 기능 고도화, bm25 기반 메시지 검색을 통해 메시지 건수를 기준으로 session 연관성을 점수화 정렬
- [ ] 사용자와 interaction을 위한 UI Builtin 도구 구현
  - [ ] visualizeData: 기본적인 시각화, bar / chart (MCPResponse w/ ui-resource)
    - type: 'hor-bar' | 'ver-bar' | 'chart'
    - data: Data[]
  - [ ] promptUser: 선택지, 기입형의 사용자 prompt를 위한 도구 (MCPResponse / ui-resource)
    - type: optional | text | file
    - prompt: string[] | string
  - [ ] reply: 선택을 확정하기 위한 Tool (promptUser의 ui-resource의 선택에 따라 uiaction으로 처리, MessageRenderer 참고)
    - answer: string
- [ ] Agent Mode 구현, AI Service의 Tool Use를 강제하도록 설정
  - ai-service의 각 서비스 provider의 구현을 검토하여 API호출 시 AI 응답에 tool use를 강제할 수 있는지 검토 필요
  - ChatContext에 agentic mode boolean state를 추가 및 관련 제어를 위한 hook 속성 추가
  - Chat UI에 Agentic Mode 토글 버튼으로 이상태를 제어하고,이에 따라 ai service가 항상 tool use를 강제하도록 (agentic mode === true) 코드 업데이트
