import { render } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom/vitest';
import type { MCPContent } from '@/lib/mcp';
import type { Message } from '@/models/chat';
import { AgentMessageRenderer } from '../AgentMessageRenderer';

const uiResourceRendererMock = vi.fn();

vi.mock('@mcp-ui/client', () => ({
  basicComponentLibrary: {},
  remoteButtonDefinition: {},
  remoteTextDefinition: {},
  remoteCardDefinition: {},
  remoteImageDefinition: {},
  remoteStackDefinition: {},
  UIResourceRenderer: (props: unknown) => {
    uiResourceRendererMock(props);
    return <div data-testid="ui-resource" />;
  },
}));

vi.mock('@/hooks/use-rust-backend', () => ({
  useRustBackend: () => ({
    openExternalUrl: vi.fn(),
    downloadMediaFile: vi.fn(),
  }),
}));

vi.mock('@/context/AgentChatContext', () => ({
  useAgentChatActions: () => ({
    submit: vi.fn(),
  }),
}));

vi.mock('@/context/AgentSessionContext', () => ({
  useAgentSessionState: () => ({
    session: { id: 'test-session', assistant: { id: 'test-assistant' } },
  }),
}));

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
    debug: vi.fn(),
  }),
}));

vi.mock('next-themes', () => ({
  useTheme: () => ({
    resolvedTheme: 'light',
  }),
}));

const resourceContent: MCPContent[] = [
  {
    type: 'resource',
    resource: {
      uri: 'ui://resource',
      mimeType: 'text/html',
      text: '<div>ui</div>',
    },
  } as MCPContent,
];

const message = {
  id: 'msg-ui',
  role: 'assistant',
  content: resourceContent,
} as Message;

describe('AgentMessageRenderer UI resource iframe sizing', () => {
  beforeEach(() => {
    uiResourceRendererMock.mockReset();
  });

  it('uses fixed 384px height when expandResources is false', () => {
    render(
      <AgentMessageRenderer content={resourceContent} message={message} />,
    );

    expect(uiResourceRendererMock).toHaveBeenCalledWith(
      expect.objectContaining({
        htmlProps: expect.objectContaining({
          style: expect.objectContaining({
            height: '384px',
            maxHeight: '80vh',
          }),
        }),
      }),
    );
  });

  it('drops the 384px height cap when expandResources is true', () => {
    render(
      <AgentMessageRenderer
        content={resourceContent}
        message={message}
        expandResources
      />,
    );

    const htmlProps = uiResourceRendererMock.mock.calls[0]?.[0]?.htmlProps as {
      style?: Record<string, string>;
    };

    expect(htmlProps.style?.height).toBeUndefined();
    expect(htmlProps.style?.maxHeight).toBeUndefined();
    expect(htmlProps.style).toEqual(
      expect.objectContaining({
        width: '100%',
        minHeight: '200px',
      }),
    );
  });
});
