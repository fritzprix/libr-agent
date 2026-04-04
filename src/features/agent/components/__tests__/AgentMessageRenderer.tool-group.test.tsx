import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom/vitest';
import type { MCPContent } from '@/lib/mcp';
import type { Message } from '@/models/chat';
import { AgentMessageRenderer } from '../AgentMessageRenderer';

const toolGroupBlockMock = vi.fn();

vi.mock('../AgentMessageRenderer/components/AgentToolGroupBlock', () => ({
  AgentToolGroupBlock: (props: unknown) => {
    toolGroupBlockMock(props);
    const { isLast } = props as { isLast: boolean };

    return <div data-testid="tool-group-block">{String(isLast)}</div>;
  },
}));

vi.mock('@mcp-ui/client', () => ({
  basicComponentLibrary: {},
  remoteButtonDefinition: {},
  remoteTextDefinition: {},
  remoteCardDefinition: {},
  remoteImageDefinition: {},
  remoteStackDefinition: {},
  UIResourceRenderer: () => <div data-testid="ui-resource">ui resource</div>,
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

vi.mock('@/lib/backend', () => ({
  executeUiTauriAction: vi.fn(),
  handleUserToolCall: vi.fn(),
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

describe('AgentMessageRenderer tool group ordering', () => {
  it('treats a tool group as last after UI-resource text filtering', () => {
    toolGroupBlockMock.mockReset();

    const content: MCPContent[] = [
      {
        type: 'resource',
        resource: {
          uri: 'ui://resource',
          mimeType: 'text/html',
          text: '<div>ui</div>',
        },
      } as MCPContent,
      {
        type: 'tool_call',
        id: 'call-1',
        name: 'test_tool',
        arguments: '{}',
      } as MCPContent,
      {
        type: 'text',
        text: 'This text is intentionally hidden when a UI resource exists.',
      } as MCPContent,
    ];

    const message = {
      id: 'msg-1',
      role: 'assistant',
      content,
    } as Message;

    render(<AgentMessageRenderer content={content} message={message} />);

    expect(screen.getByTestId('tool-group-block')).toHaveTextContent('true');
    expect(toolGroupBlockMock.mock.calls[0]?.[0]).toEqual(
      expect.objectContaining({ isLast: true }),
    );
  });
});
