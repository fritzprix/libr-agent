import '@testing-library/jest-dom';
import { vi } from 'vitest';
import type { Message } from '@/models/chat';
import type { GroupedMessage } from '@/hooks/useMessageGrouping';
import {
  installMockResizeObserver,
  resetAgentChatMessagesHarness,
  type AgentChatMessagesTestHarness,
} from './AgentChatMessages.compaction-setup';

export function setupScrollFollowHarness() {
  const {
    virtuosoMock,
    scrollToIndexMock,
    sessionState,
    chatState,
    hasVirtuosoHandle,
  } = vi.hoisted(() => ({
    virtuosoMock: vi.fn(),
    scrollToIndexMock: vi.fn(),
    sessionState: {
      session: { id: 'session-1', assistant: { name: 'Agent' } },
    },
    chatState: {
      messages: [] as Message[],
      workflowStatus: 'idle' as 'idle' | 'busy',
    },
    hasVirtuosoHandle: { current: true },
  }));

  const groupedMessagesMock: GroupedMessage[] = [];
  const resizeObserverCallbacks = { current: [] as ResizeObserverCallback[] };

  installMockResizeObserver(resizeObserverCallbacks);

  const harness: AgentChatMessagesTestHarness = {
    virtuosoMock,
    scrollToIndexMock,
    sessionState,
    chatState,
    hasVirtuosoHandle,
  };

  const resetHarness = () => {
    resetAgentChatMessagesHarness({
      harness,
      groupedMessagesMock,
      resizeObserverCallbacks,
    });
  };

  return {
    virtuosoMock,
    scrollToIndexMock,
    sessionState,
    chatState,
    hasVirtuosoHandle,
    groupedMessagesMock,
    resizeObserverCallbacks,
    harness,
    resetHarness,
  };
}
