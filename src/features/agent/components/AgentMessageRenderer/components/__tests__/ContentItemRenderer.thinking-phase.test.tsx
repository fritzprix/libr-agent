import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { ContentItemRenderer } from '../ContentItemRenderer';
import type { Message } from '@/models/chat';
import type { MCPThinkingContent } from '@/lib/mcp';

vi.mock('../../../shared', () => ({
  ThinkingBubble: ({ isStreaming }: { isStreaming: boolean }) => (
    <div data-testid="thinking-bubble" data-streaming={String(isStreaming)}>
      Thinking Content
    </div>
  ),
}));

vi.mock('../../AgentMarkdownText', () => ({
  MarkdownText: () => <div>Markdown</div>,
}));

vi.mock('../AgentToolGroupBlock', () => ({
  AgentToolGroupBlock: () => <div>ToolGroup</div>,
}));

describe('ContentItemRenderer thinking streamingPhase', () => {
  const thinkingItem: MCPThinkingContent = {
    type: 'thinking',
    thinking: 'Analyzing data...',
  };

  it('passes isStreaming=true when streamingPhase is thinking', () => {
    const message: Message = {
      id: 'msg-1',
      sessionId: 'session-1',
      threadId: 'session-1',
      role: 'assistant',
      content: [thinkingItem],
      isStreaming: true,
      streamingPhase: 'thinking',
    };

    const defaultProps = {
      itemKey: 'item-0',
      expandResources: false,
      resourceRefs: { current: {} },
      markdownComponents: {},
      remoteDomProps: {} as unknown as NonNullable<
        React.ComponentProps<typeof ContentItemRenderer>['remoteDomProps']
      >,
      supportedContentTypes: [],
      htmlProps: {} as unknown as NonNullable<
        React.ComponentProps<typeof ContentItemRenderer>['htmlProps']
      >,
      themeStyleTag: null,
      themeKey: 'dark',
      onUIAction: vi.fn(),
      onLinkClick: vi.fn(),
    };

    render(
      <ContentItemRenderer
        {...defaultProps}
        item={thinkingItem}
        message={message}
        toolResultsMap={new Map()}
        isLast={true}
      />,
    );

    const bubble = screen.getByTestId('thinking-bubble');
    expect(bubble).toHaveAttribute('data-streaming', 'true');
  });

  it('passes isStreaming=false when message transitions to generating phase (thinking finished)', () => {
    const message: Message = {
      id: 'msg-1',
      sessionId: 'session-1',
      threadId: 'session-1',
      role: 'assistant',
      content: [
        thinkingItem,
        { type: 'text', text: 'Here is the answer' },
      ],
      isStreaming: true,
      streamingPhase: 'generating',
    };

    const defaultProps = {
      itemKey: 'item-0',
      expandResources: false,
      resourceRefs: { current: {} },
      markdownComponents: {},
      remoteDomProps: {} as unknown as NonNullable<
        React.ComponentProps<typeof ContentItemRenderer>['remoteDomProps']
      >,
      supportedContentTypes: [],
      htmlProps: {} as unknown as NonNullable<
        React.ComponentProps<typeof ContentItemRenderer>['htmlProps']
      >,
      themeStyleTag: null,
      themeKey: 'dark',
      onUIAction: vi.fn(),
      onLinkClick: vi.fn(),
    };

    render(
      <ContentItemRenderer
        {...defaultProps}
        item={thinkingItem}
        message={message}
        toolResultsMap={new Map()}
        isLast={false}
      />,
    );

    const bubble = screen.getByTestId('thinking-bubble');
    expect(bubble).toHaveAttribute('data-streaming', 'false');
  });
});
