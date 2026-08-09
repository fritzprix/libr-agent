# Navigation Guide — Developer View

> LibrAgent의 UI 라우트와 내부 코드 구조 매핑. 일반 사용자는 [사용자 문서](../user/README.md)를 참고하세요.

---

## Routes & Code Mapping

### `/` — Agent Workspace (Default)

**Route**: `/` (루트)
**Component**: `src/features/agent/`
**Providers**: `ChatProvider`, `AgentSessionProvider`, `AgentChatProvider`

실제 채팅 인터페이스. 에이전트 세션의 대화, 도구 호출, 생각 과정을 표시합니다.

| 하위 컴포넌트      | 위치                                                 |
| ------------------ | ---------------------------------------------------- |
| 채팅 입력          | `features/agent/components/AgentChatInput.tsx`       |
| 도구 호출 UI       | `features/agent/components/AgentToolCallDetails.tsx` |
| 토큰 참조 드롭다운 | `features/agent/components/InputTokenDropdown.tsx`   |
| 입력 토큰 훅       | `features/agent/hooks/useInputToken.ts`              |

---

### `/agent/draft` — 새 세션 초안

**Route**: `/agent/draft`
**Component**: 세션 초안 화면

첫 메시지 전송 전 에이전트 구성 (모델 선택, 시스템 프롬프트 등)을 설정합니다.
전송하면 `/agent/:sessionId`로 전환됩니다.

---

### `/agent/:sessionId` — 활성 세션

**Route**: `/agent/:sessionId`
**Component**: `features/agent/` (세션별 인스턴스)

각 세션은 독립적인 `MCPServiceProxy` 인스턴스를 가집니다. 프론트엔드는 세션 관리를 직접 하지 않으며, Rust 백엔드의 `agent:event`로 상태 업데이트를 수신합니다.

**세션 상태 관리**:

- `ChatProvider` — 채팅 메시지 상태
- `AgentSessionProvider` — 세션 수명주기
- `AgentChatProvider` — 세션별 채팅 상태

---

### `/assistants` — 어시스턴트 프로필

**Route**: `/assistants`
**Component**: `features/assistants/`

커스텀 어시스턴트 생성/관리. 각 어시스턴트는:

- 시스템 프롬프트
- 사용 가능한 내장 도구 (builtin capabilities)
- 외부 MCP 서버

**API**: Tauri 명령어는 `src/lib/backend/assistants.ts`에서 호출

---

### `/playbooks` — 워크플로우 템플릿

**Route**: `/playbooks`
**Component**: `features/playbooks/`

재사용 가능한 워크플로우 템플릿 관리.
**API**: `playbook__listPlaybooks`, `playbook__createPlaybook` 등

---

### `/history` / `/history/search` — 세션 아카이브

**Route**: `/history`, `/history/search`
**Component**: `features/history/`

과거 세션 목록 및 검색.
**API**: `history__listSessions`, `history__searchHistory`, `history__readSession`

---

### `/settings` — 시스템 설정

**Route**: `/settings`
**Component**: `features/settings/`

**탭 구조**:

| 탭             | Component                   | 주요 설정                    |
| -------------- | --------------------------- | ---------------------------- |
| General        | `GeneralSettings.tsx`       | 언어, 스킬 디렉터리          |
| AI & Models    | `AIModelsSettings.tsx`      | API 키, Default/Fallback LLM |
| Chat Interface | `ChatInterfaceSettings.tsx` | 컨텍스트 크기 등             |
| System         | `SystemSettings.tsx`        | 시스템 옵션                  |
| Advanced       | `AdvancedSettings.tsx`      | 셸 런타임 PATH 등            |
| Experimental   | `ExperimentalSettings.tsx`  | 실험 기능                    |

**Settings Service**: `src/lib/services/settings-service.ts`
**Settings Interface**: `SystemSettings` 타입 정의

---

### `/mcp-servers` — MCP 서버 관리

**Route**: `/mcp-servers`
**Component**: `features/mcp-servers/`

외부 MCP 서버 연결 관리. 현재 네비게이션에 직접 표시되지 않을 수 있으나 (로케일 키만 남아있을 수 있음), API는 활성 상태입니다.

**관련 백엔드**:

- `src-tauri/src/mcp/` — MCP 통합
- `MCPServiceProxy` — 세션 고립형 도구 라우터
- `HttpSessionManager` — HTTP MCP 세션 관리
- `SessionMCPManager` — 세션별 MCP 관리

---

### `/scheduled-tasks` — 자동화

**Route**: `/scheduled-tasks`
**Component**: `features/scheduled-tasks/`

크론 기반 에이전트 자동화 스케줄 관리.
**API**: `scheduled_task__createScheduledTask`, `scheduled_task__listScheduledTasks` 등

---

## Key Files Reference

### 프론트엔드

```
src/
├── lib/backend/              # Tauri 명령어 래퍼
│   ├── safeInvoke.ts         # 중앙 에러 처리 호출
│   ├── messages.ts           # 메시지 타입/처리
│   ├── assistants.ts         # 어시스턴트 API
│   ├── playbooks.ts          # 플레이북 API
│   ├── history.ts            # 히스토리 API
│   └── settings.ts           # 설정 API
├── features/agent/           # 메인 채팅 기능
│   ├── components/           # AgentChatInput 등
│   └── hooks/                # useInputToken 등
├── features/settings/        # 설정 UI
├── features/assistants/      # 어시스턴트 관리
├── features/history/         # 세션 히스토리
├── features/playbooks/       # 플레이북 관리
└── context/                  # 전역 Context providers
```

### 백엔드

```
src-tauri/src/
├── agent/                    # 에이전트 오케스트레이션
│   ├── llm/completion.rs     # LLM 호출 파이프라인
│   ├── references/           # @type:name 참조 시스템
│   └── mod.rs                # agent 모듈 진입점
├── commands/                 # Tauri 명령어 핸들러
├── mcp/                      # MCP 통합
│   ├── session_isolation_config.rs
│   └── ...
├── config.rs                 # 설정 (타임아웃 등)
└── main.rs                   # 진입점
```

---

## State Management

### React Context Flow

```
App
└── ChatProvider (전역 채팅 상태)
    └── AgentSessionProvider (세션 수명주기)
        └── AgentChatProvider (세션별 채팅)
            └── AgentChatInput / AgentChatDisplay
```

### Event-Driven Updates

프론트엔드는 완전히 반응형입니다. 에이전트 상태 변경은 Tauri 이벤트 (`agent:event`)로 전달되며, 프론트엔드가 직접 상태를 관리하지 않습니다.

---

## Related

- [개발자 시작 가이드](./getting-started-dev.md)
- [문제 해결 (개발자용)](./troubleshooting-dev.md)
- [프로젝트 가이드 (agents.md)](../../agents.md)
