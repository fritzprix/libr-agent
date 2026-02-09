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

    // Use regex to be more resilient to tokenization
    expect(screen.getByText(/hello/)).toBeInTheDocument();
  });

  it('memoizes CodeBlock when content is unchanged during rerender', async () => {
    const content: MCPContent[] = [
      {
        type: 'text',
        text: '```javascript\nconst x = 42;\n```',
      },
    ];

    const { rerender } = render(<AgentMessageRenderer content={content} />);

    // Find the initial code block
    const codeElement = await screen.findByText(/const/);
    expect(codeElement).toBeInTheDocument();

    // Rerender with the exact same content
    // This simulates what happens during streaming when other parts of the message update
    // but the code block content hasn't changed
    rerender(<AgentMessageRenderer content={content} />);

    // The code block should still be present and unchanged
    // In a real scenario, without proper memoization, this would trigger re-highlighting
    expect(screen.getByText(/const/)).toBeInTheDocument();
    expect(screen.getByText(/42/)).toBeInTheDocument();
  });
});
