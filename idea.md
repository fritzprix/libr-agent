# Agent Runtime for Multi Agent Orchestration

AI Agent가 on-demand로 assistant를 탐색하고 지시를 내리고 상호간에 대화를 할 수 있는 수단을 제공

## 주요 개념

### Agent process의 실행

- 별도의 rust program으로 경량의 agent process를 구동
- pipe를 생성하여 stdio를 통해 RPC를 통해 child process <-> parent process 간 통신
  - ## 주요 interface
