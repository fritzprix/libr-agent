import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import '@testing-library/jest-dom/vitest';
import { AgentToolCallGroup } from '../AgentToolCallGroup';
import type { Message, ToolCall } from '@/models/chat';

// Mock dependencies
vi.mock('@/hooks/use-settings', () => ({
  useSettings: () => ({
    value: { display: { toolDetailLevel: 'developer' } },
  }),
}));

// Mock child component to verify props
vi.mock('../ToolCallCompactItem', () => ({
  ToolCallCompactItem: ({
    toolCall,
    toolResult,
  }: {
    toolCall: ToolCall;
    toolResult?: Message;
  }) => (
    <div data-testid="tool-item">
      <span data-testid="call-id">{toolCall.id}</span>
      <span data-testid="result-id">{toolResult?.id || 'no-result'}</span>
    </div>
  ),
}));

// Helper to create tool call
const makeToolCall = (id: string): ToolCall => ({
  id,
  type: 'function',
  function: { name: 'test_tool', arguments: '{}' },
});

// Helper to create tool result message
const makeToolResult = (id: string, callId: string): Message =>
  ({
    id,
    sessionId: 'session-1',
    threadId: 'thread-1',
    role: 'tool',
    content: [{ type: 'text', text: 'result' }],
    tool_call_id: callId,
  }) as Message;

const mockMessage = { id: 'msg-1', role: 'assistant' } as Message;

describe('AgentToolCallGroup Rendering', () => {
  it('renders visible calls with correct results when collapsed (slice logic)', () => {
    // Create 5 calls and 5 results
    const calls = Array.from({ length: 5 }, (_, i) =>
      makeToolCall(`call-${i}`),
    );
    const results = calls.map((c, i) => makeToolResult(`result-${i}`, c.id));

    // Render with visibleCount = 3
    render(
      <AgentToolCallGroup
        message={mockMessage}
        toolGroup={{ calls }}
        toolResults={results}
        visibleCount={3}
      />,
    );

    // Should only see the last 3 items: indices 2, 3, 4
    const items = screen.getAllByTestId('tool-item');
    expect(items).toHaveLength(3);

    // Verify correct mapping
    // Item 0 (call-2) -> result-2
    expect(items[0]).toHaveTextContent('call-2');
    expect(items[0]).toHaveTextContent('result-2');

    // Item 1 (call-3) -> result-3
    expect(items[1]).toHaveTextContent('call-3');
    expect(items[1]).toHaveTextContent('result-3');

    // Item 2 (call-4) -> result-4
    expect(items[2]).toHaveTextContent('call-4');
    expect(items[2]).toHaveTextContent('result-4');
  });

  it('renders all calls with correct results when expanded', () => {
    // Create 5 calls and 5 results
    const calls = Array.from({ length: 5 }, (_, i) =>
      makeToolCall(`call-${i}`),
    );
    const results = calls.map((c, i) => makeToolResult(`result-${i}`, c.id));

    render(
      <AgentToolCallGroup
        message={mockMessage}
        toolGroup={{ calls }}
        toolResults={results}
        visibleCount={3}
      />,
    );

    // Click "Show All"
    const toggle = screen.getByText(/Show All/i);
    fireEvent.click(toggle);

    // Should see all 5 items
    const items = screen.getAllByTestId('tool-item');
    expect(items).toHaveLength(5);

    // Verify specifically call-0 -> result-0 (which was hidden before)
    expect(items[0]).toHaveTextContent('call-0');
    expect(items[0]).toHaveTextContent('result-0');
  });

  it('opts the tool group container out of browser scroll anchoring', () => {
    const calls = Array.from({ length: 2 }, (_, i) => makeToolCall(`call-${i}`));
    const results = calls.map((c, i) => makeToolResult(`result-${i}`, c.id));

    const { container } = render(
      <AgentToolCallGroup
        message={mockMessage}
        toolGroup={{ calls }}
        toolResults={results}
        visibleCount={1}
      />,
    );

    expect(container.firstElementChild).toHaveStyle({ overflowAnchor: 'none' });
  });
});
