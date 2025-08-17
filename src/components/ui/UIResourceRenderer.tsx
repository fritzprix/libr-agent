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

  // HTML 리소스에 스크립트 주입 (iframe 링크 클릭 처리)
  if (mcpUIResource.mimeType?.startsWith('text/html') && mcpUIResource.text) {
    mcpUIResource.text += `
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
