# Agentic Workflow Backend 검토

## 문제점

- 현재 구조에서 Agentic Workflow에 대한 관리를 React Frontend에서 처리하고 있으며 이는 다음과 같은 문제를 발생시킴
  - Frontend에서 세션 전환에 따라 Agentic Flow의 Interruption이 발생되고 이로 인해서 불완전한 상태가 발생함
  - 따라서 이러한 불완전한 상태가 발생되는 것을 방지하기 위해서는 사용자의 세션 전환을 방지하는 것이 필요한데 이는 사용자 경험을 열화시킴

## 해결 방안

- Agentic Workflow를 처리하기 위한 독립적 별도의 Thread가 필요함
- LibrAgent는 단순히 Frontend로써 정보의 시각화와 상호작용을 위한 역할만 수행하게됨

## 추가적인 고려사항

- 더 나아가 Single Agent => Multi Agent로 확장할 경우 이러한 Agentic Workflow를 위한 Backend는 추가적인 장점이 있음
- 이러한 확장성 높은 Generic Agentic Workflow를 개발하기 위해서 다음의 요구사항을 제안

### 요구사항

- 외부에서 Agent 정의 / 환경 정의 / Data 통합을 Runtime에 주입할 수 있어야 함
  - Agent 정의
    - name
    - system prompt
    - tools
  - Environment
    - Agent에 AI 기능을 공급할 AI Model API => Completion service
    - Agent의 도구 호출을 실제 실행할 환경을 제공 => Tool service
    - Agent의 응답 생성 및 Message History를 저장하고 관리하기 위한 Data Backend => Data service
    - 응답의 출력을 전달할 Output endpoint
- IPC / ITC를 통해 Frontend에서 쉽게 Agentic Workflow를 통합할 수 있어야 함
  - 가령 React Frontend에서 동적으로 위 요소들을 주입하고 Agentic Workflow를 실행할 수 있어야 함
  - 동시에 필요한 경우 해당 Agentic Flow의 응답을 실시간으로 전달받고 이를 Frontend에서 표시할 수 있어야함
