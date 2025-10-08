# Workspace의 Terminal 및 Code Interpreter 도구 개선

- 현재
  - execute_* tool은 모두 synchronous operation만을 지원하고 있음, timeout을 지원하고 있으나 기다리는 동안 AI Agent는 아무것도 하지 못함
  - workspace를 제공하고 있으나 terminal의 환경 변수 등 설정이 유지되지 않음, 이로 인해 점진적 terminal 환경을 셋업하기 어려움

- ToBe
  - Terminal session을 관리하는 기능을 추가
  - open_new_terminal() => terminal_id
  - execute_*에서 반드시 이 terminal_id를 사용하도록
  - execute_*는 async 옵션을 통해 sync 동작 및 async 동작 모두를 지원
  - terminal 의 출력을 읽어 올 수 있는 read_terminal_output을 제공, 비동기적 처리 결과를 ai agent가 확인할 수 있음
  - close_terminal, list_terminal 등 session 관리 도구 추가

