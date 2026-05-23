// Shared fixtures, factories, and Virtuoso mock helpers for
// AgentChatMessages compaction test suites.
//
// IMPORTANT: This module contains no vi.* calls. Vitest's hoisting rules
// require all vi.mock() and vi.hoisted() declarations to reside in the
// test file that uses them.

import React from 'react';
import type { Message } from '@/models/chat';
import type { GroupedMessage } from '@/hooks/useMessageGrouping';

// ---------------------------------------------------------------------------
// Static message fixtures
// ---------------------------------------------------------------------------

export const baseMessage: Message = {
  id: 'assistant-1',
  sessionId: 'session-1',
  threadId: 'session-1',
  role: 'assistant',
  content: [{ type: 'text', text: 'Tool call message' }],
};

export const groupedToolMessages: Message[] = [
  {
    id: 'earlier-user',
    sessionId: 'session-1',
    threadId: 'session-1',
    role: 'user',
    content: [{ type: 'text', text: 'Earlier user message' }],
  },
  baseMessage,
  {
    id: 'tool-1',
    sessionId: 'session-1',
    threadId: 'session-1',
    role: 'tool',
    tool_call_id: 'call-1',
    content: [{ type: 'text', text: 'Tool result' }],
  },
];

// ---------------------------------------------------------------------------
// GroupedMessage factories
// ---------------------------------------------------------------------------

/**
 * Returns the default compact tool group entry used as the initial
 * groupedMessagesMock state in most tests.
 */
export function makeCompactToolGroupEntry(): GroupedMessage {
  return {
    type: 'tool_group',
    message: baseMessage,
    messages: [baseMessage],
    coveredMessageIds: ['assistant-1', 'tool-1'],
    toolGroup: {
      calls: [
        {
          id: 'call-1',
          type: 'function',
          function: { name: 'agent__compactSessionContext', arguments: '{}' },
        },
      ],
      results: [],
    },
  };
}

/** Wraps a message in a single-type GroupedMessage. */
export function makeSingleGroupEntry(message: Message): GroupedMessage {
  return {
    type: 'single',
    message,
    messages: [message],
    coveredMessageIds: [message.id],
  } as GroupedMessage;
}

/** Creates a streaming assistant message (default text: 'streaming output'). */
export function makeStreamingMessage(text = 'streaming output'): Message {
  return {
    ...baseMessage,
    id: 'assistant-stream',
    content: [{ type: 'text', text }],
    isStreaming: true,
  };
}

/** Returns a single grouped entry wrapping a streaming assistant message. */
export function makeStreamingGroupEntry(
  text = 'streaming output',
): GroupedMessage {
  return makeSingleGroupEntry(makeStreamingMessage(text));
}

// ---------------------------------------------------------------------------
// Virtuoso mock implementation
// ---------------------------------------------------------------------------

interface VirtuosoMockProps {
  components?: {
    Footer?: ({ context }: { context: unknown }) => React.ReactElement | null;
    Header?: ({ context }: { context: unknown }) => React.ReactElement | null;
    List?: (props: {
      children: React.ReactNode;
      context: unknown;
      style?: React.CSSProperties;
    }) => React.ReactElement | null;
    Scroller?: React.ComponentType<
      React.ComponentPropsWithoutRef<'div'>
    > | null;
  };
  context: unknown;
  data: GroupedMessage[];
  itemContent: (
    index: number,
    item: GroupedMessage,
  ) => React.ReactElement | null;
}

/**
 * Applies the standard Virtuoso mock implementation to a vitest mock function.
 * The mock renders list items through the component's own itemContent and
 * Virtuoso component slots so that test assertions can inspect real DOM output.
 */
export function applyVirtuosoMockImpl(virtuosoMock: {
  mockImplementation: (
    fn: (props: VirtuosoMockProps) => React.ReactElement,
  ) => void;
}): void {
  virtuosoMock.mockImplementation(
    ({
      components,
      context,
      data,
      itemContent,
    }: VirtuosoMockProps): React.ReactElement => {
      const Scroller = (components?.Scroller ?? 'div') as React.ElementType<{
        children?: React.ReactNode;
      }>;
      const List = components?.List;

      const content = (
        <>
          {components?.Header ? <components.Header context={context} /> : null}
          {data.map((item, index) => (
            <div key={item.message.id}>{itemContent(index, item)}</div>
          ))}
          {components?.Footer ? <components.Footer context={context} /> : null}
        </>
      );

      return (
        <Scroller>
          {List ? (
            <List context={context} style={{}}>
              {content}
            </List>
          ) : (
            content
          )}
        </Scroller>
      );
    },
  );
}
