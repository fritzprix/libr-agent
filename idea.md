# Assistants backend as a Tool (including minor improvement over assistant backend)

- Assistant의 CRUD를 MCP Tool로 제공하여 Assistant가 Assistant를 생성할 수 있도록 지원
- Assistant의 backend를 Local DB를 기본으로 제공하고 선택적으로 (SettingPage에 Server URL을 설정하면) REST API를 통해 원격의 서버(Agent Hub)를 backend로 사용할 수 있도록 지원
  - 현재 API 명세가 정의되어 있지 않으며 클라이언트 설계 시 정의하고 이것을 나중에 서버 구현에 사용하면 됨
  - Agent Hub의 URL을 Setting에 입력하면 Local DB는 Backup용으로 사용되며 AgentHub로 부터 사용된 Assistant를 저장하여 서버가 장애가 있을 경우 일종의 Offline Fallback으로 제공될 수 있도록 함
- 
