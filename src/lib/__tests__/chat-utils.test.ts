import { describe, it, expect } from 'vitest';
import {
  createSystemMessage,
  createUserMessage,
  createToolMessage,
  createToolSuccessMessage,
  createToolMessagePair,
} from '../chat-utils';
import { stringToMCPContentArray } from '../utils';

describe('chat-utils', () => {
  const sessionId = 'session-123';
  const threadId = 'thread-456';
  const assistantId = 'assistant-789';

  describe('createSystemMessage', () => {
    it('should create a system message with default threadId', () => {
      const text = 'System instruction';
      const message = createSystemMessage(text, sessionId);

      expect(message.role).toBe('system');
      expect(message.content).toEqual(stringToMCPContentArray(text));
      expect(message.sessionId).toBe(sessionId);
      expect(message.threadId).toBe(sessionId);
      expect(message.id).toBeDefined();
    });

    it('should create a system message with specific threadId and assistantId', () => {
      const text = 'System instruction';
      const message = createSystemMessage(text, sessionId, threadId, assistantId);

      expect(message.threadId).toBe(threadId);
      expect(message.assistantId).toBe(assistantId);
    });

    it('should create a system message with source', () => {
      const message = createSystemMessage('test', sessionId, undefined, undefined, 'ui');
      expect(message.source).toBe('ui');
    });
  });

  describe('createUserMessage', () => {
    it('should create a user message', () => {
      const text = 'User input';
      const message = createUserMessage(text, sessionId);

      expect(message.role).toBe('user');
      expect(message.content).toEqual(stringToMCPContentArray(text));
      expect(message.sessionId).toBe(sessionId);
      expect(message.threadId).toBe(sessionId);
      expect(message.id).toBeDefined();
    });
  });

  describe('createToolMessage', () => {
    it('should create a tool message', () => {
      const content = stringToMCPContentArray('Tool result');
      const toolCallId = 'call-123';
      const message = createToolMessage(content, toolCallId, sessionId);

      expect(message.role).toBe('tool');
      expect(message.content).toEqual(content);
      expect(message.tool_call_id).toBe(toolCallId);
      expect(message.sessionId).toBe(sessionId);
    });

    it('should throw error if toolCallId is missing', () => {
      expect(() => {
        // @ts-expect-error Testing runtime check
        createToolMessage([], '', sessionId);
      }).toThrow('tool_call_id is required');
    });
  });

  describe('createToolSuccessMessage', () => {
    it('should create a tool success message', () => {
      const result = 'Success';
      const toolCallId = 'call-123';
      const message = createToolSuccessMessage(result, toolCallId, sessionId);

      expect(message.role).toBe('tool');
      expect(message.content[0].type).toBe('text');
      expect((message.content[0] as { text: string }).text).toBe('✅ Success');
      expect(message.tool_call_id).toBe(toolCallId);
    });
  });

  describe('createToolMessagePair', () => {
    it('should create a pair of tool call and result messages', () => {
      const toolName = 'testTool';
      const params = { key: 'value' };
      const result = stringToMCPContentArray('Result');
      const toolCallId = 'call-123';

      const [callMsg, resultMsg] = createToolMessagePair(
        toolName,
        params,
        result,
        toolCallId,
        sessionId
      );

      // Verify call message
      expect(callMsg.role).toBe('assistant');
      expect(callMsg.tool_calls).toHaveLength(1);
      expect(callMsg.tool_calls![0].id).toBe(toolCallId);
      expect(callMsg.tool_calls![0].function.name).toBe(toolName);
      expect(JSON.parse(callMsg.tool_calls![0].function.arguments)).toEqual(params);
      expect(callMsg.sessionId).toBe(sessionId);
      expect(callMsg.createdAt).toBeDefined();

      // Verify result message
      expect(resultMsg.role).toBe('tool');
      expect(resultMsg.tool_call_id).toBe(toolCallId);
      expect(resultMsg.content).toEqual(result);
      expect(resultMsg.sessionId).toBe(sessionId);
      expect(resultMsg.createdAt).toBeDefined();

      // Verify timestamps logic (result should be after call)
      expect(resultMsg.createdAt!.getTime()).toBeGreaterThanOrEqual(
        callMsg.createdAt!.getTime()
      );
    });

    it('should propagate source to both messages', () => {
        const [callMsg, resultMsg] = createToolMessagePair(
            'tool', {}, [], 'id', sessionId, undefined, undefined, 'ui'
        );
        expect(callMsg.source).toBe('ui');
        expect(resultMsg.source).toBe('ui');
    });
  });
});
