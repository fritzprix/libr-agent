import React, { useCallback, useEffect, useRef } from 'react';
import type { UIResource } from '@/models/chat';
import {
  UIResourceRenderer as ExternalUIResourceRenderer,
  UIActionResult,
} from '@mcp-ui/client';
import { getLogger } from '@/lib/logger';
import { useRustBackend } from '@/hooks/use-rust-backend';

const logger = getLogger('UIResourceRenderer');

export interface UIResourceRendererProps {
  resource: UIResource | UIResource[];
  onUIAction?: (action: UIActionResult) => Promise<unknown>;
}

/**
 * mcp-ui 표준을 준수하는 UIResourceRenderer
 * @mcp-ui/client의 UIResourceRenderer를 직접 사용하여 표준 동작 보장
 */
const UIResourceRenderer: React.FC<UIResourceRendererProps> = ({
  resource,
  onUIAction,
}) => {
  const { openExternalUrl, downloadWorkspaceFile, exportAndDownloadZip } =
    useRustBackend();
  const containerRef = useRef<HTMLDivElement>(null);

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

  // HTML 리소스에 스크립트 주입 (iframe 링크 클릭 처리 및 postMessage 수신)
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

      // Listen for postMessage events from the iframe content
      window.addEventListener('message', function(event) {
        // Re-send iframe messages to parent frame
        if (event.source === window) {
          window.parent.postMessage(event.data, '*');
        }
      });
    </script>
  `;
  }

  // 통합된 다운로드 처리 함수
  const handleDownload = useCallback(
    async (
      toolName: string,
      params: Record<string, unknown>,
      targetWindow?: Window | null,
    ) => {
      const getIframeWindow = () => {
        if (containerRef.current) {
          const iframe = containerRef.current.querySelector('iframe');
          return iframe?.contentWindow;
        }
        return null;
      };

      let result;
      try {
        if (toolName === 'download_workspace_file') {
          result = await downloadWorkspaceFile(params.filePath as string);
          logger.info('File downloaded successfully', { params, result });
        } else if (toolName === 'export_and_download_zip') {
          result = await exportAndDownloadZip(
            params.files as string[],
            params.packageName as string,
          );
          logger.info('ZIP exported and downloaded successfully', {
            params,
            result,
          });
        } else {
          return; // Not a download tool
        }

        // Send success message back to the correct iframe
        const destination = targetWindow || getIframeWindow();
        if (destination) {
          destination.postMessage(
            { type: 'download_complete', success: true, result },
            '*',
          );
        }
      } catch (error) {
        logger.error('Failed to process download', { toolName, params, error });

        // Send error message back to the correct iframe
        const destination = targetWindow || getIframeWindow();
        if (destination) {
          destination.postMessage(
            {
              type: 'download_complete',
              success: false,
              error: error instanceof Error ? error.message : String(error),
            },
            '*',
          );
        }
      }
    },
    [downloadWorkspaceFile, exportAndDownloadZip],
  );

  // postMessage 이벤트 리스너 (iframe에서 온 메시지 처리)
  const handlePostMessage = useCallback(
    async (event: MessageEvent) => {
      // `tool` 타입 메시지만 처리
      if (event.data.type === 'tool' && event.source) {
        await handleDownload(
          event.data.payload.toolName,
          event.data.payload.params,
          event.source as Window,
        );
      }
    },
    [handleDownload],
  );

  useEffect(() => {
    window.addEventListener('message', handlePostMessage);
    return () => window.removeEventListener('message', handlePostMessage);
  }, [handlePostMessage]);

  // mcp-ui 표준 onUIAction 핸들러
  const handleUIAction = onUIAction
    ? onUIAction
    : async (action: UIActionResult): Promise<void> => {
        logger.info('Default UI action handling', {
          type: action.type,
          payload: action.payload,
        });

        if (action.type === 'link') {
          const url = action.payload.url;
          try {
            await openExternalUrl(url);
            logger.info('External URL opened successfully', { url });
          } catch (error) {
            logger.error(
              'Failed to open external URL via Tauri, falling back to window.open',
              { url, error },
            );
            window.open(url, '_blank', 'noopener,noreferrer');
          }
        } else if (action.type === 'tool') {
          // 다운로드 액션을 통합 핸들러로 전달
          await handleDownload(action.payload.toolName, action.payload.params);
        }
      };

  return (
    <div ref={containerRef}>
      <ExternalUIResourceRenderer
        resource={mcpUIResource}
        onUIAction={handleUIAction}
      />
    </div>
  );
};

export default UIResourceRenderer;
