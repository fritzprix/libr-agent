import { describe, it, expect } from 'vitest';
import { messageToRustMessage, rustMessageToMessage, Message, RustMessage, ToolCall } from '../chat';

describe('Message conversions', () => {
  const now = new Date();
  const nowTs = now.getTime();

  const toolCall: ToolCall = {
    id: 'tc1',
    type: 'function',
    function: {
      name: 'test_tool',
      arguments: '{}',
    },
  };

  const fullMessage: Message = {
    id: 'msg1',
    sessionId: 'session1',
    threadId: 'session1',
    role: 'assistant',
    content: [{ type: 'text', text: 'Hello' }],
    tool_calls: [toolCall],
    tool_call_id: 'tc1',
    isStreaming: false,
    thinking: 'thinking...',
    thinkingSignature: 'sig1',
    thinkingTime: 100,
    assistantId: 'asst1',
    attachments: [],
    tool_use: { id: 'tu1', name: 'tool1', input: {} },
    usage: { promptTokens: 10, completionTokens: 20, totalTokens: 30, cachedPromptTokens: 5 },
    createdAt: now,
    updatedAt: now,
    source: 'assistant',
    error: undefined,
    metadata: { retryCount: 1 },
  };

  it('converts Message to RustMessage correctly', () => {
    const rustMsg = messageToRustMessage(fullMessage);

    expect(rustMsg.id).toBe(fullMessage.id);
    expect(rustMsg.sessionId).toBe(fullMessage.sessionId);
    expect(rustMsg.role).toBe(fullMessage.role);
    expect(rustMsg.content).toEqual(fullMessage.content);

    // CamelCase check
    expect(rustMsg.toolCalls).toEqual(fullMessage.tool_calls);
    expect(rustMsg.toolCallId).toBe(fullMessage.tool_call_id);
    expect(rustMsg.toolUse).toEqual(fullMessage.tool_use);
    expect(rustMsg.usage).toEqual(fullMessage.usage);

    // Timestamp check
    expect(rustMsg.createdAt).toBe(nowTs);
    expect(rustMsg.updatedAt).toBe(nowTs);
  });

  it('handles missing tool_calls correctly', () => {
    const msg: Message = {
      ...fullMessage,
      tool_calls: undefined,
    };
    const rustMsg = messageToRustMessage(msg);
    expect(rustMsg.toolCalls).toBeUndefined();
  });

  it('converts RustMessage back to Message correctly', () => {
    const rustMsg: RustMessage = {
      id: 'msg1',
      sessionId: 'session1',
      role: 'assistant',
      content: [{ type: 'text', text: 'Hello' }],
      toolCalls: [toolCall],
      toolCallId: 'tc1',
      toolUse: { id: 'tu1', name: 'tool1', input: {} },
      usage: { promptTokens: 10, completionTokens: 20, totalTokens: 30, cachedPromptTokens: 5 },
      createdAt: nowTs,
      updatedAt: nowTs,
      source: 'assistant',
    };

    const msg = rustMessageToMessage(rustMsg);

    expect(msg.id).toBe(rustMsg.id);
    expect(msg.tool_calls).toEqual(rustMsg.toolCalls);
    expect(msg.tool_call_id).toBe(rustMsg.toolCallId);
    expect(msg.tool_use).toEqual(rustMsg.toolUse);
    expect(msg.usage).toEqual(rustMsg.usage);
    expect(msg.createdAt?.getTime()).toBe(nowTs);
  });

  it('preserves channel source and metadata from RustMessage', () => {
    const rustMsg: RustMessage = {
      id: 'channel-msg',
      sessionId: 'session1',
      role: 'user',
      content: [{ type: 'text', text: '<channel source="alerts">build failed</channel>' }],
      createdAt: nowTs,
      updatedAt: nowTs,
      source: 'channel',
      metadata: {
        channel: {
          serverName: 'alerts',
          meta: { severity: 'high' },
        },
      },
    };

    const msg = rustMessageToMessage(rustMsg);

    expect(msg.source).toBe('channel');
    expect(msg.metadata).toEqual(rustMsg.metadata);
  });

  it('preserves backend-emitted non-ui sources from RustMessage', () => {
    const rustMsg: RustMessage = {
      id: 'scheduled-msg',
      sessionId: 'session1',
      role: 'user',
      content: [{ type: 'text', text: 'Scheduled run' }],
      createdAt: nowTs,
      updatedAt: nowTs,
      source: 'scheduled_task',
    };

    const msg = rustMessageToMessage(rustMsg);

    expect(msg.source).toBe('scheduled_task');
  });

  it('drops unknown RustMessage sources instead of trusting them blindly', () => {
    const rustMsg = {
      id: 'weird-msg',
      sessionId: 'session1',
      role: 'user',
      content: [{ type: 'text', text: 'Unexpected source' }],
      createdAt: nowTs,
      updatedAt: nowTs,
      source: 'weird-source',
    } as unknown as RustMessage;

    const msg = rustMessageToMessage(rustMsg);

    expect(msg.source).toBeUndefined();
  });

  it('updatedAt falls back to createdAt if missing', () => {
    const createdAt = new Date('2023-01-01');
    const msg: Message = {
      ...fullMessage,
      createdAt,
      updatedAt: undefined,
    };

    const rustMsg = messageToRustMessage(msg);
    expect(rustMsg.createdAt).toBe(createdAt.getTime());
    expect(rustMsg.updatedAt).toBe(createdAt.getTime());
  });

  it('handles both createdAt and updatedAt missing (uses now)', () => {
      const msg: Message = {
          ...fullMessage,
          createdAt: undefined,
          updatedAt: undefined
      };

      const before = Date.now();
      const rustMsg = messageToRustMessage(msg);
      const after = Date.now();

      expect(rustMsg.createdAt).toBeGreaterThanOrEqual(before);
      expect(rustMsg.createdAt).toBeLessThanOrEqual(after);
      expect(rustMsg.updatedAt).toBe(rustMsg.createdAt);
  });

  it('handles number timestamps in Message (runtime robustness)', () => {
      // simulate runtime object with number timestamps while keeping type safety
      const msg = {
          ...fullMessage,
          createdAt: nowTs as unknown,
          updatedAt: undefined, // should fall back to createdAt (nowTs)
      } as Message;

      const rustMsg = messageToRustMessage(msg);
      expect(rustMsg.createdAt).toBe(nowTs);
      expect(rustMsg.updatedAt).toBe(nowTs);
  });
});
