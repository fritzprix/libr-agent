# Built-In Tools의 관리 구조 개선

- session에 따라 도구의 상태를 관리해야 되는 경우가 많음
- 이를 공통으로 제공하기 위해 #BuiltInService에 아래와 같이 type 추가

  ```ts
  interface SwitchSessionOptions {
    sessionId: string;
  }
  export interface BuiltInService {
    metadata: ServiceMetadata;
    listTools: () => MCPTool[];
    executeTool: (toolCall: ToolCall) => Promise<MCPResponse<unknown>>;
    loadService?: () => Promise<void>;
    unloadService?: () => Promise<void>;
    getServiceContext?: (options?: ServiceContextOptions) => Promise<string>;
    switchSession: ({ sessionId }) => Promise<void>;
  }
  ```

- 모든 built-in 도구들은 새롭게 session에 따라 상태를 관리할 수 있는 interface를 추가 (혹은 기존에 유사한 기능이 있었으면 이를 switchSession으로 migration)

## Session의 전환과 switchSession의 처리

- useSession hook을 BuiltInToolProvider에서 구독
- session의 변화 시 useEffect에서 switchSession을 일괄적으로 호출

## Legacy의 제거

- 기존 setContext / setServerContext 등의 별도 interface가 있었으나 새로운 switchSession으로 migration을 통해 하나의 일관된 interface로 통합

## getServiceContext의 개선

- `getServiceContext -> Promise<string>`은 각 도구의 state를 가져오는데 제한적
- string과 함께 structured state를 가져올 수 있도록 getServiceContext의 return type을 변경한다

  ```ts
  interface ServiceContext<T> {
    contextPrompt: string;
    structuredState: T;
  }
  ```

- `getServiceContext -> Promise<ServiceContext<T>>`로 변경하고 각 built-in tools의 getServiceContext 구현을 수정한다.
- 이 structuredState와 관련하여 현재 UI에서 상태 시각화를 담당하는 아래 코드들을 검토할 것
  - #file:ChatPlanningPanel.tsx
  - #file:WorkspaceFilesPanel.tsx
  - #file:SessionFilesPopover.tsx
- 각 도구의 상태를 UI에 시각화 하기 위해 BuiltInToolContextType에는 serviceContext: Record<string, unknown>을 추가
  - 이 상태는 아래 UI 요소에서 시각화 하기 위한 도구 상태를 가져가기 위해 사용됨
    - #file:ChatPlanningPanel.tsx
    - #file:WorkspaceFilesPanel.tsx
    - #file:SessionFilesPopover.tsx

- BuiltInToolProvider에서는 buildToolPrompt가 호출될 때 getServiceContext를 호출하게 되며 이때 반환된 값 중 structuredState를 이용하여 언급한 serviceContext를 갱신함.
- serviceContext를 쉽게 구독할 수 있도록 useServiceContext<T>(serviceName)를 추가
