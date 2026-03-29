import { render, screen } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import '@testing-library/jest-dom/vitest';
import { AgentMessageRenderer } from '../AgentMessageRenderer';
import type { MCPContent } from '@/lib/mcp';
import type { Message } from '@/models/chat';
import { readLocalFileAsBase64 } from '@/lib/backend/workspace';

vi.mock('@/lib/backend/workspace', () => ({
  readLocalFileAsBase64: vi.fn(),
}));

// Mock contexts and hooks
vi.mock('@/hooks/use-rust-backend', () => ({
  useRustBackend: () => ({
    openExternalUrl: vi.fn(),
  }),
}));

vi.mock('@/hooks/use-settings', () => ({
  useSettings: () => ({
    value: {
      toolCallGroupVisibleCount: 4,
    },
    update: vi.fn(),
    isLoading: false,
    error: null,
  }),
}));

vi.mock('@/context/AgentChatContext', () => ({
  useAgentChatActions: () => ({
    submit: vi.fn(),
    injectMessages: vi.fn(),
  }),
}));

vi.mock('@/context/AgentSessionContext', () => ({
  useAgentSessionState: () => ({
    session: { id: 'test-session', assistant: { id: 'test-assistant' } },
  }),
}));

vi.mock('next-themes', () => ({
  useTheme: () => ({
    resolvedTheme: 'light',
  }),
}));

vi.mock('@/hooks/useClipboard', () => ({
  useClipboard: () => ({
    copied: false,
    copyToClipboard: vi.fn(),
  }),
}));

// Mock logger
vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
    debug: vi.fn(),
  }),
}));

describe('AgentMessageRenderer', () => {
  it('renders text content with code block', async () => {
    const content: MCPContent[] = [
      {
        type: 'text',
        text: 'Here is some code:\n```javascript\nconsole.log("hello");\n```',
      },
    ];

    render(<AgentMessageRenderer content={content} />);

    // Check if code block is rendered
    const codeElement = await screen.findByText('console');
    expect(codeElement).toBeInTheDocument();

    expect(screen.getByText(/hello/)).toBeInTheDocument();
  });

  it('renders tool groups correctly using AgentToolGroupBlock', () => {
    const content: MCPContent[] = [
      {
        type: 'tool_call',
        id: 'call-1',
        name: 'test_tool',
        arguments: '{}',
      },
    ];

    const message = {
      id: 'msg-1',
      role: 'assistant',
      content: [],
    } as unknown as Message;

    render(<AgentMessageRenderer content={content} message={message} />);

    // Check if tool name is rendered
    expect(screen.getByText('test_tool')).toBeInTheDocument();
    // Check if "Tool Executions" header exists (from AgentToolCallGroup)
    expect(screen.getByText(/Tool Executions/)).toBeInTheDocument();
  });

  it('lazily materializes file URIs to data URLs for image rendering', async () => {
    vi.mocked(readLocalFileAsBase64).mockResolvedValue('dG9vbC1pbWFnZQ==');
    const content: MCPContent[] = [
      {
        type: 'image',
        uri: 'file:///tmp/tool-output.png',
        mimeType: 'image/png',
      },
    ];
    const message = {
      id: 'msg-1',
      sessionId: 'test-session',
      threadId: 'test-session',
      role: 'assistant',
      content,
      createdAt: new Date(),
      updatedAt: new Date(),
    } as Message;

    render(<AgentMessageRenderer content={content} message={message} />);

    const image = await screen.findByAltText('Tool output');
    expect(readLocalFileAsBase64).toHaveBeenCalledWith(
      'test-session',
      'file:///tmp/tool-output.png',
    );
    expect(image).toHaveAttribute(
      'src',
      'data:image/png;base64,dG9vbC1pbWFnZQ==',
    );
  });
});
