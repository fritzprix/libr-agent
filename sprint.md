# Sprint Log

## 2025-10-10

- [ ] Workspace MCP 도구에 execute_shell에 async 실행 지원 추가
  - async execute일 경우 process에 대한 정보를 응답
  - background로 실행할 경우 이 process의 상태 관리 기능을 추가하고
  - get_service_context에 진행중인 process의 요약을 제공
  - poll_process (or better name)을 통해 process의 상태 정보 및 완료 시 terminal output을 읽어올 수 있음
- [ ] Assistant를 검색하고 assistant가 assistant에게 task를 부여할 수 있는 MCP Server 구현
  - list_assistant
    - pagination 지원
  - search_assistant
    - query: string
    - assistant의 description을 기반으로 bm25 matching 검색 제공
  - spawn_assistant
    - assistant_id: string
    - query: string
  - poll_assistant
- [ ] light weight assistant runner 구현을 위한 기획
  - 현재 프로젝트의 타입과 호환되는 light weight assistant runner program을 rust로 작성하고자 한다.
  - assistant, tools, workspace, playbook, built-in tool context
  - stdio를 통한 RPC 통신
    - spawned assistant -> main process: get_service_context
    - spawned assistant -> main process: 
