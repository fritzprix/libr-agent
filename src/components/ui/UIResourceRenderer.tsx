import React from 'react';
import { UIResource } from '@/models/chat';
import { UIResourceRenderer as ExternalUIResourceRenderer } from '@mcp-ui/client';

// 문서 스펙 기반 UIAction 타입 정의
export type UIAction =
  | {
      type: 'tool';
      payload: { toolName: string; params: Record<string, unknown> };
      messageId?: string;
    }
  | {
      type: 'intent';
      payload: { intent: string; params: Record<string, unknown> };
      messageId?: string;
    }
  | { type: 'prompt'; payload: { prompt: string }; messageId?: string }
  | { type: 'notify'; payload: { message: string }; messageId?: string }
  | { type: 'link'; payload: { url: string }; messageId?: string };

export interface UIResourceRendererProps {
  resource: UIResource | UIResource[];
  onUIAction?: (action: UIAction) => void;
}

/**
 * 외부 라이브러리 직접 사용으로 단순화된 wrapper
 * 배열 입력 시 첫 번째 리소스만 렌더링
 */
const UIResourceRenderer: React.FC<UIResourceRendererProps> = ({
  resource,
  onUIAction,
}) => {
  const first = Array.isArray(resource) ? resource[0] : resource;
  if (!first) return null;

  // 외부 라이브러리 요구사항에 맞춘 최소 매핑
  const adaptedResource = {
    ...first,
    // 필수 필드 보강
    name: first.uri ?? 'ui-resource',
    uri: first.uri ?? 'ui://inline',
  };

  // Promise 반환하는 onUIAction wrapper
  const adaptedOnUIAction = onUIAction
    ? async (result: unknown): Promise<void> => {
        // 간단한 변환: 외부 라이브러리 result를 UIAction으로 변환
        if (result && typeof result === 'object' && 'type' in result) {
          const candidate = result as {
            type: string;
            payload?: unknown;
            messageId?: string;
          };
          onUIAction({
            type: candidate.type as UIAction['type'],
            payload: candidate.payload as Record<string, unknown> | undefined,
            messageId: candidate.messageId,
          } as UIAction);
        }
      }
    : undefined;

  return (
    <ExternalUIResourceRenderer
      resource={adaptedResource}
      onUIAction={adaptedOnUIAction}
    />
  );
};

export default UIResourceRenderer;
