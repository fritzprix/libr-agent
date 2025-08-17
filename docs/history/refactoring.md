# Refactoring Plan

## 작업의 목적

Tauri v2 환경에서 MCP UI 리소스 내의 외부 링크가 클릭되어도 외부 브라우저가 열리지 않는 문제를 해결하여, 사용자가 UI 리소스에서 제공하는 링크를 정상적으로 외부 브라우저에서 열 수 있도록 개선한다.

## 현재의 상태 / 문제점

### 1. 권한 설정 누락

- `tauri.conf.json`에 shell.open 권한이 설정되지 않아 외부 브라우저 오픈 API 사용 불가

### 2. 백엔드 명령어 부재

- `src-tauri/src/lib.rs`에 외부 URL을 여는 Tauri 명령어가 정의되지 않음
- `src/hooks/use-rust-backend.ts`에 외부 URL 오픈 함수가 없음

### 3. 프론트엔드 링크 처리 문제

- `MessageBubbleRouter.tsx`에서 `window.open` 사용 시 Tauri 환경에서 외부 브라우저가 열리지 않음
- iframe 내부의 링크 클릭이 부모 컨텍스트로 전달되지 않음

### 4. 로그 분석 결과

```
"uiResource": {
  "text": "<a href=\"http://localhost:3000/game/cube_...\" target=\"_blank\">Click to Play!</a>",
  "uri": "ui://game-link/cube_..."
}
```

- 하드코딩된 localhost URL로 인한 배포 환경 호환성 문제

## 변경 이후의 상태 / 해결 판정 기준

### 성공 기준

1. MCP UI 리소스 내 링크 클릭 시 외부 브라우저에서 정상 오픈
2. iframe 내부 링크가 postMessage를 통해 부모로 전달
3. 브라우저/Tauri 환경 모두에서 호환 동작
4. 중앙화된 로깅을 통한 링크 오픈 추적

### 검증 방법

1. `pnpm tauri dev` 실행 후 MCP UI 리소스 링크 클릭 테스트
2. 외부 브라우저 정상 오픈 확인
3. 로그에서 에러 없이 처리되는지 확인

## 수정이 필요한 코드 및 수정부분의 코드 스니핏

### 1. `src-tauri/tauri.conf.json` - 권한 설정 추가

```json
{
  // ...기존 설정...
  "allowlist": {
    "shell": {
      "open": true
    }
  }
  // ...기존 설정...
}
```

### 2. `src-tauri/src/lib.rs` - 외부 URL 오픈 명령어 추가

```rust
#[tauri::command]
async fn open_external_url(url: String) -> Result<(), String> {
    // URL 유효성 검증
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("Only HTTP/HTTPS URLs are allowed".to_string());
    }

    // 외부 브라우저에서 열기
    std::process::Command::new("xdg-open")
        .arg(&url)
        .spawn()
        .map_err(|e| format!("Failed to open URL: {}", e))?;

    Ok(())
}

// invoke_handler에 추가
.invoke_handler(tauri::generate_handler![
    // ...기존 핸들러들...
    open_external_url  // 새로 추가
])
```

### 3. `src/hooks/use-rust-backend.ts` - 외부 URL 오픈 함수 추가

```typescript
// External URL handling 섹션 추가
const openExternalUrl = async (url: string): Promise<void> => {
  return safeInvoke<void>('open_external_url', { url });
};

return {
  // ...기존 exports...

  // External URL handling
  openExternalUrl,

  // ...기존 exports...
};
```

### 4. `src/features/chat/MessageBubbleRouter.tsx` - 링크 처리 로직 수정

```typescript
import { useRustBackend } from '@/hooks/use-rust-backend';

const MessageBubbleRouter: React.FC<MessageBubbleRouterProps> = ({
  message,
}) => {
  const { executeToolCall } = useUnifiedMCP();
  const { submit } = useChatContext();
  const { openExternalUrl } = useRustBackend();

  // ...기존 코드...

  case 'link': {
    const url = action.payload.url;
    if (!url) return;

    try {
      await openExternalUrl(url);
      logger.info('External URL opened successfully', { url });
    } catch (error) {
      logger.error('Failed to open external URL', { url, error });
      // 폴백: 브라우저 환경에서만 window.open 사용
      if (typeof window !== 'undefined') {
        window.open(url, '_blank', 'noopener,noreferrer');
      }
    }
    break;
  }

  // ...기존 코드...
};
```

### 5. `src/components/ui/UIResourceRenderer.tsx` - iframe 링크 클릭 처리 추가

```typescript
// HTML 리소스에 스크립트 주입
if (adaptedResource.mimeType?.startsWith('text/html') && adaptedResource.text) {
  adaptedResource.text += `
    <script>
      document.addEventListener('click', function(e) {
        var a = e.target.closest && e.target.closest('a');
        if (a && a.href && (a.href.startsWith('http://') || a.href.startsWith('https://'))) {
          e.preventDefault();
          window.parent.postMessage({ type: 'link', payload: { url: a.href } }, '*');
        }
      });
    </script>
  `;
}
```

## 구현 순서

1. `tauri.conf.json` 권한 설정
2. `lib.rs`에 Tauri 명령어 추가
3. `use-rust-backend.ts`에 함수 추가
4. `MessageBubbleRouter.tsx` 링크 처리 로직 수정
5. `UIResourceRenderer.tsx` iframe 링크 처리 추가
6. 빌드 및 테스트

---

## 추가 개선 사항: MCP-UI 표준 준수 강화

### 목적

현재 UIResourceRenderer가 불필요한 타입 변환을 수행하고 있어, mcp-ui 공식 표준을 더욱 충실히 준수하도록 코드를 단순화하고 유지보수성을 향상시킨다.

### 현재 상태 / 문제점

#### 1. 불필요한 타입 변환

- `UIResourceRenderer.tsx`에서 `name` 필드를 임의로 추가하고 있음 (mcp-ui 스펙에 없음)
- 복잡한 타입 변환 로직으로 인한 코드 복잡성 증가

#### 2. mcp-ui 표준 미준수

- `@mcp-ui/client`의 UIResourceRenderer를 직접 사용하지 않고 불필요한 래핑 수행
- UIAction 타입 변환에서 불필요한 복잡성 도입

### MCP-UI 표준 준수 - 변경 결과 및 판정 기준

#### MCP-UI 개선 성공 기준

1. mcp-ui 공식 스펙을 정확히 준수하는 UIResource 타입 사용
2. 불필요한 타입 변환 로직 제거로 코드 단순화
3. `@mcp-ui/client` 라이브러리의 표준 동작 보장
4. 향후 mcp-ui 라이브러리 업데이트 시 자동 호환성 확보

#### MCP-UI 개선 검증 방법

1. UIResource 렌더링이 기존과 동일하게 동작하는지 확인
2. UIAction 처리가 정상적으로 작동하는지 테스트
3. mcp-ui 표준 스펙 준수 여부 검증

### MCP-UI 표준 준수 - 수정 코드

#### `src/components/ui/UIResourceRenderer.tsx` - mcp-ui 표준 준수 개선

```typescript
import React from 'react';
import type { UIResource } from '@/models/chat';
import { UIResourceRenderer as ExternalUIResourceRenderer } from '@mcp-ui/client';
import { getLogger } from '@/lib/logger';

const logger = getLogger('UIResourceRenderer');

// mcp-ui 표준 UIAction 타입 (공식 스펙 그대로)
export type UIAction =
  | { type: 'tool', payload: { toolName: string, params: Record<string, unknown> }, messageId?: string }
  | { type: 'intent', payload: { intent: string, params: Record<string, unknown> }, messageId?: string }
  | { type: 'prompt', payload: { prompt: string }, messageId?: string }
  | { type: 'notify', payload: { message: string }, messageId?: string }
  | { type: 'link', payload: { url: string }, messageId?: string };

export interface UIResourceRendererProps {
  resource: UIResource | UIResource[];
  onUIAction?: (action: UIAction) => void;
}

/**
 * mcp-ui 표준을 준수하는 UIResourceRenderer
 * @mcp-ui/client의 UIResourceRenderer를 직접 사용하여 표준 동작 보장
 */
const UIResourceRenderer: React.FC<UIResourceRendererProps> = ({
  resource,
  onUIAction,
}) => {
  // 배열인 경우 첫 번째 리소스만 사용 (mcp-ui 표준)
  const targetResource = Array.isArray(resource) ? resource[0] : resource;
  
  if (!targetResource) {
    logger.warn('No UI resource provided');
    return null;
  }

  // mcp-ui 표준 UIResource 형태로 정규화 (불필요한 필드 제거)
  const mcpUIResource = {
    uri: targetResource.uri || 'ui://inline',
    mimeType: targetResource.mimeType || 'text/html',
    text: targetResource.text,
    blob: targetResource.blob,
  };

  // mcp-ui 표준 onUIAction 핸들러 (직접 전달, 변환 없음)
  const handleUIAction = onUIAction
    ? async (action: UIAction): Promise<void> => {
        logger.info('UI action received', { 
          type: action.type, 
          payload: action.payload,
          messageId: action.messageId 
        });
        onUIAction(action);
      }
    : undefined;

  return (
    <ExternalUIResourceRenderer
      resource={mcpUIResource}
      onUIAction={handleUIAction}
    />
  );
};

export default UIResourceRenderer;
```

### 개선사항 상세

#### 1. 타입 정확성 향상

- `name` 필드 제거: mcp-ui 스펙에 정의되지 않은 필드 제거
- UIAction 타입을 mcp-ui 공식 스펙과 정확히 일치하도록 정의

#### 2. 코드 단순화

- 불필요한 타입 변환 로직 제거
- `@mcp-ui/client`의 표준 동작을 그대로 활용

#### 3. 로깅 개선

- 중앙화된 로깅 시스템 활용
- UIAction 처리 시 상세한 로그 정보 제공

#### 4. 유지보수성 향상

- mcp-ui 라이브러리 업데이트 시 자동 호환성 확보
- 표준 준수로 인한 디버깅 용이성 증대

### 구현 우선순위

1. **높음**: UIResourceRenderer 표준 준수 개선
2. **중간**: 불필요한 타입 변환 로직 제거
3. **낮음**: 로깅 및 에러 처리 개선

이 개선사항을 통해 SynapticFlow의 MCP-UI 연동이 더욱 안정적이고 표준에 충실한 구현이 됩니다.
