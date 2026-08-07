import { render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom/vitest';
import type { MCPContent } from '@/lib/mcp';
import type { Message } from '@/models/chat';
import { AgentMessageRenderer } from '../AgentMessageRenderer';
import { UI_RESOURCE_THEME_MARKER } from '../AgentMessageRenderer/utils/injectUiResourceTheme';

const uiResourceRendererMock = vi.fn();
const themeState = vi.hoisted(() => ({
  resolvedTheme: 'dark' as string | undefined,
}));

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
    resolvedTheme: themeState.resolvedTheme,
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

describe('AgentMessageRenderer UI resource theme', () => {
  beforeEach(() => {
    uiResourceRendererMock.mockReset();
    themeState.resolvedTheme = 'dark';
  });

  it('injects host theme CSS into rawHtml and sets iframe color-scheme', () => {
    render(
      <AgentMessageRenderer content={resourceContent} message={message} />,
    );

    const props = uiResourceRendererMock.mock.calls[0]?.[0] as {
      resource: { text?: string };
      htmlProps: {
        iframeProps?: {
          style?: { colorScheme?: string; backgroundColor?: string };
        };
      };
    };

    expect(props.resource.text).toContain(UI_RESOURCE_THEME_MARKER);
    expect(props.resource.text).toContain('color-scheme: dark');
    expect(props.resource.text).toContain('--background: oklch(0.145 0 0)');
    expect(props.resource.text).toContain('<div>ui</div>');
    expect(props.htmlProps.iframeProps?.style?.colorScheme).toBe('dark');
    expect(props.htmlProps.iframeProps?.style?.backgroundColor).toBe(
      'oklch(0.145 0 0)',
    );
  });

  it('waits for resolvedTheme before mounting the iframe', () => {
    themeState.resolvedTheme = undefined;

    render(
      <AgentMessageRenderer content={resourceContent} message={message} />,
    );

    expect(uiResourceRendererMock).not.toHaveBeenCalled();
    expect(screen.queryByTestId('ui-resource')).not.toBeInTheDocument();
  });
});

describe('AgentMessageRenderer UI resource iframe sizing', () => {
  beforeEach(() => {
    uiResourceRendererMock.mockReset();
    themeState.resolvedTheme = 'dark';
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
