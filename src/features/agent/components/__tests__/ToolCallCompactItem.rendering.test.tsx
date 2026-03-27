import { render, cleanup, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi, afterEach } from 'vitest';
import '@testing-library/jest-dom/vitest';
import { ToolCallCompactItem } from '../ToolCallCompactItem';
import type { ToolCall, Message } from '@/models/chat';

// Mock dependencies
const mockT = vi.fn((key) => key);
vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: mockT }),
}));

let mockDetailLevel = 'developer';
vi.mock('@/hooks/use-settings', () => ({
  useSettings: () => ({
    value: { display: { toolDetailLevel: mockDetailLevel } },
  }),
}));

// Mock AgentToolCallDetails to avoid deep rendering issues and dependencies
vi.mock('../AgentToolCallDetails', () => ({
  AgentToolCallDetails: () => <div data-testid="tool-details" />,
}));

const makeToolCall = (id: string): ToolCall => ({
  id,
  type: 'function',
  function: { name: 'test_tool', arguments: '{}' },
});

const makeToolResult = (id: string, hasError = false): Message => ({
  id: `res-${id}`,
  role: 'tool',
  content: [{ type: 'text', text: 'result' }],
  tool_call_id: id,
  error: hasError ? 'Something went wrong' : undefined,
} as unknown as Message);

describe('ToolCallCompactItem Rendering and Transitions', () => {
  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  it('auto-expands in developer mode when an error occurs', () => {
    mockDetailLevel = 'developer';
    const toolCall = makeToolCall('call-1');
    
    // First render: no result
    const { rerender, queryByTestId } = render(
      <ToolCallCompactItem toolCall={toolCall} />
    );
    expect(queryByTestId('tool-details')).not.toBeInTheDocument();

    // Second render: result with error
    const toolResultWithError = makeToolResult('call-1', true);
    rerender(<ToolCallCompactItem toolCall={toolCall} toolResult={toolResultWithError} />);
    
    // Should be expanded now
    expect(queryByTestId('tool-details')).toBeInTheDocument();
  });

  it('does NOT auto-expand in simple mode when an error occurs', () => {
    mockDetailLevel = 'simple';
    const toolCall = makeToolCall('call-2');
    
    const { rerender, queryByTestId } = render(
      <ToolCallCompactItem toolCall={toolCall} />
    );

    const toolResultWithError = makeToolResult('call-2', true);
    rerender(<ToolCallCompactItem toolCall={toolCall} toolResult={toolResultWithError} />);
    
    expect(queryByTestId('tool-details')).not.toBeInTheDocument();
  });

  it('stays collapsed when switching from simple to developer mode if transition occurred in simple mode', () => {
    // 1. Simple mode, no error
    mockDetailLevel = 'simple';
    const toolCall = makeToolCall('call-3');
    const { rerender, queryByTestId } = render(
      <ToolCallCompactItem toolCall={toolCall} />
    );

    // 2. Simple mode, error occurs
    const toolResultWithError = makeToolResult('call-3', true);
    rerender(<ToolCallCompactItem toolCall={toolCall} toolResult={toolResultWithError} />);
    expect(queryByTestId('tool-details')).not.toBeInTheDocument();

    // 3. Switch to developer mode
    mockDetailLevel = 'developer';
    rerender(<ToolCallCompactItem toolCall={toolCall} toolResult={toolResultWithError} />);
    
    // Critical: Should NOT be expanded because the sentinel synced during simple mode
    expect(queryByTestId('tool-details')).not.toBeInTheDocument();
  });

  it('does NOT re-trigger expansion on subsequent renders if no new transition occurred', () => {
    mockDetailLevel = 'developer';
    const toolCall = makeToolCall('call-4');
    
    // 1. Initial render
    const { rerender, queryByTestId, getByLabelText } = render(
      <ToolCallCompactItem toolCall={toolCall} />
    );

    // 2. Error occurs -> auto-expand
    const toolResultWithError = makeToolResult('call-4', true);
    rerender(<ToolCallCompactItem toolCall={toolCall} toolResult={toolResultWithError} />);
    expect(queryByTestId('tool-details')).toBeInTheDocument();

    // 3. User manually collapses it
    const toggleButton = getByLabelText('agentChat.toolDetails.toggleAriaLabel');
    fireEvent.click(toggleButton);
    expect(queryByTestId('tool-details')).not.toBeInTheDocument();

    // 4. Rerender with same props (simulating parent update)
    rerender(<ToolCallCompactItem toolCall={toolCall} toolResult={toolResultWithError} />);
    
    // Should stay collapsed (no new transition)
    expect(queryByTestId('tool-details')).not.toBeInTheDocument();
  });
});
