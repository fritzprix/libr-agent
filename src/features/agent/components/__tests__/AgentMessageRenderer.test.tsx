import { render, screen } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import '@testing-library/jest-dom/vitest';
import { AgentMessageRenderer } from '../AgentMessageRenderer';
import type { MCPContent } from '@/lib/mcp';

// Mock contexts and hooks
vi.mock('@/hooks/use-rust-backend', () => ({
  useRustBackend: () => ({
    openExternalUrl: vi.fn(),
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
    // Since prism-react-renderer splits text into tokens, we might need to be flexible
    // But usually "console" and "log" are tokens.

    // We can just check that the container exists or some part of text exists.
    const codeElement = await screen.findByText('console');
    expect(codeElement).toBeInTheDocument();

    expect(screen.getByText(/hello/)).toBeInTheDocument();
  });
});
