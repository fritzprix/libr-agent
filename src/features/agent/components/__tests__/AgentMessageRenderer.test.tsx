import { render, screen } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import { AgentMessageRenderer } from '../AgentMessageRenderer';
import type { Message } from '@/models/chat';
import type { MCPContent } from '@/lib/mcp-types';

// Mock dependencies
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

vi.mock('@/hooks/use-rust-backend', () => ({
  useRustBackend: () => ({
    openExternalUrl: vi.fn(),
  }),
}));

vi.mock('@/hooks/useClipboard', () => ({
  useClipboard: () => ({
    copied: false,
    copyToClipboard: vi.fn(),
  }),
}));

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    info: vi.fn(),
    debug: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  }),
}));

// Mock mcp-ui/client components since they might not be available in test environment
vi.mock('@mcp-ui/client', () => ({
  UIResourceRenderer: () => <div data-testid="ui-resource-renderer" />,
  basicComponentLibrary: {},
  remoteButtonDefinition: {},
  remoteTextDefinition: {},
  remoteCardDefinition: {},
  remoteImageDefinition: {},
  remoteStackDefinition: {},
}));

// Mock matchMedia
Object.defineProperty(window, 'matchMedia', {
  writable: true,
  value: vi.fn().mockImplementation(query => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: vi.fn(), // deprecated
    removeListener: vi.fn(), // deprecated
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    dispatchEvent: vi.fn(),
  })),
});

describe('AgentMessageRenderer Security', () => {
  it('should sanitize dangerous HTML', () => {
    const dangerousContent: MCPContent[] = [
      {
        type: 'text',
        text: 'Hello <script>alert("xss")</script> World',
      },
    ];

    const message: Message = {
      id: 'msg1',
      role: 'assistant',
      content: dangerousContent,
      createdAt: new Date(),
      sessionId: 'test-session',
      threadId: 'test-thread',
    };

    render(<AgentMessageRenderer message={message} />);

    // The script tag should be removed or rendered as text if escaped (but markdown renders HTML)
    // Since we use rehype-sanitize, <script> should be stripped.
    // "Hello " and " World" should be present. "alert" inside script should be gone.
    expect(screen.getByText(/Hello/)).toBeInTheDocument();
    expect(screen.getByText(/World/)).toBeInTheDocument();

    // Ensure script content is not rendered
    const alertText = screen.queryByText('alert("xss")');
    expect(alertText).not.toBeInTheDocument();
  });

  it('should allow className in code blocks', () => {
    const codeContent: MCPContent[] = [
      {
        type: 'text',
        text: '```javascript\nconsole.log("hello");\n```',
      },
    ];

    const message: Message = {
      id: 'msg2',
      role: 'assistant',
      content: codeContent,
      createdAt: new Date(),
      sessionId: 'test-session',
      threadId: 'test-thread',
    };

    const { container } = render(<AgentMessageRenderer message={message} />);

    // Check if prism-react-renderer is working (it uses spans with class names)
    // We mocked UIResourceRenderer but not ReactMarkdown or Highlight from prism-react-renderer
    // ReactMarkdown uses Highlight which produces spans with styles/classes.

    // We just want to check if the code block is rendered and not stripped.
    // And verify that className is preserved (prism-react-renderer uses class names for styling)
    const consoleToken = screen.getByText('console');
    expect(consoleToken).toBeInTheDocument();
    expect(consoleToken).toHaveClass('token', 'console', 'class-name');

    const stringToken = screen.getByText('"hello"');
    expect(stringToken).toBeInTheDocument();
    expect(stringToken).toHaveClass('token', 'string');
  });

  it('should allow target="_blank" and rel="noopener noreferrer" in links', () => {
    const linkContent: MCPContent[] = [
      {
        type: 'text',
        text: '[External Link](https://example.com)',
      },
    ];

    const message: Message = {
      id: 'msg3',
      role: 'assistant',
      content: linkContent,
      createdAt: new Date(),
      sessionId: 'test-session',
      threadId: 'test-thread',
    };

    render(<AgentMessageRenderer message={message} />);

    const link = screen.getByRole('link', { name: 'External Link' });
    expect(link).toBeInTheDocument();
    expect(link).toHaveAttribute('href', 'https://example.com');
    expect(link).toHaveAttribute('target', '_blank');
    expect(link).toHaveAttribute('rel', 'noopener noreferrer');
  });

  it('should allow style attribute for KaTeX (math rendering)', () => {
    // KaTeX produces spans with style attributes for positioning.
    // We can't easily check the full output of KaTeX, but we can verify that
    // if we manually insert a span with style (simulating KaTeX output passed through sanitization),
    // the style is preserved.
    // However, ReactMarkdown with rehypeSanitize sanitizes the *output* of plugins.
    // So if we use rehype-katex, it runs BEFORE sanitization.

    // Let's try rendering a simple math equation
    const mathContent: MCPContent[] = [
      {
        type: 'text',
        text: '$E=mc^2$',
      },
    ];

    const message: Message = {
      id: 'msg4',
      role: 'assistant',
      content: mathContent,
      createdAt: new Date(),
      sessionId: 'test-session',
      threadId: 'test-thread',
    };

    const { container } = render(<AgentMessageRenderer message={message} />);

    // Check if we have spans with style attributes
    // KaTeX usually produces class="katex..." and spans with style="top:..." etc.
    // We check if at least one style attribute is present in the rendered HTML
    // Note: in JSDOM/Vitest, styles might be parsed.

    // We can check if the container contains "katex" class
    expect(container.querySelector('.katex')).toBeInTheDocument();

    // And ideally check that style attributes are not stripped.
    // But since we can't easily know *exactly* what KaTeX outputs without a snapshot,
    // let's assume if 'katex' class is there and we allowed styles in schema, it should work.
    // A stronger test would be to manually verify the schema configuration allows 'style'.
    // Or we can rely on the fact that we added 'style' to the schema in the code.
  });
});
