import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { ToolCallCompactItem } from '../ToolCallCompactItem';
import type { ToolCall, Message } from '@/models/chat';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, defaultValue?: string) => defaultValue || key,
  }),
}));

vi.mock('@/hooks/use-settings', () => ({
  useSettings: () => ({
    value: { display: { toolDetailLevel: 'simple' } },
  }),
}));

describe('ToolCallCompactItem streaming vs executing status', () => {
  const mockToolCall: ToolCall = {
    id: 'call-1',
    type: 'function',
    function: {
      name: 'bash',
      arguments: '{"command":"ls"}',
    },
  };

  it('renders tool-status-calling (Pencil) when isStreaming is true and no result', () => {
    render(
      <ToolCallCompactItem
        toolCall={mockToolCall}
        toolResult={undefined}
        isStreaming={true}
      />,
    );

    expect(screen.getByTestId('tool-status-calling')).toBeInTheDocument();
    expect(screen.queryByTestId('tool-status-executing')).not.toBeInTheDocument();
  });

  it('renders tool-status-executing (Loader2) when isStreaming is false and no result', () => {
    render(
      <ToolCallCompactItem
        toolCall={mockToolCall}
        toolResult={undefined}
        isStreaming={false}
      />,
    );

    expect(screen.getByTestId('tool-status-executing')).toBeInTheDocument();
    expect(screen.queryByTestId('tool-status-calling')).not.toBeInTheDocument();
  });

  it('renders completed check icon when toolResult is present', () => {
    const mockResult: Message = {
      id: 'res-1',
      sessionId: 'session-1',
      threadId: 'session-1',
      role: 'tool',
      content: [{ type: 'text', text: 'success' }],
    };

    render(
      <ToolCallCompactItem
        toolCall={mockToolCall}
        toolResult={mockResult}
        isStreaming={false}
      />,
    );

    expect(screen.queryByTestId('tool-status-calling')).not.toBeInTheDocument();
    expect(screen.queryByTestId('tool-status-executing')).not.toBeInTheDocument();
  });
});
