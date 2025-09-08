import { createId } from '@paralleldrive/cuid2';
import { Message } from '@/models/chat';
import { stringToMCPContentArray } from '@/lib/utils';
import { MCPContent } from '@/lib/mcp-types';

/**
 * 시스템 메시지를 생성하는 헬퍼 함수
 * plan.md 제안에 따른 중복 제거
 */
export const createSystemMessage = (
  text: string,
  sessionId: string,
  assistantId?: string,
): Message => ({
  id: createId(),
  content: stringToMCPContentArray(text),
  role: 'system',
  sessionId,
  assistantId,
});

/**
 * 사용자 메시지를 생성하는 헬퍼 함수
 */
export const createUserMessage = (
  text: string,
  sessionId: string,
  assistantId?: string,
): Message => ({
  id: createId(),
  content: stringToMCPContentArray(text),
  role: 'user',
  sessionId,
  assistantId,
});

/**
 * 스트리밍 어시스턴트 메시지를 생성하는 헬퍼 함수
 * ID를 명시적으로 고정할 수 있어 streaming 과정에서 일관성 유지
 */
export const createStreamingMessage = (
  id: string,
  content: MCPContent[],
  sessionId: string,
  assistantId?: string,
  options?: {
    thinking?: string;
    thinkingSignature?: string;
    tool_calls?: import('@/models/chat').ToolCall[];
    isStreaming?: boolean;
  },
): Message => ({
  id,
  content,
  role: 'assistant',
  sessionId,
  assistantId,
  ...options,
});

/**
 * 어시스턴트 메시지를 생성하는 헬퍼 함수
 */
export const createAssistantMessage = (
  content: MCPContent[],
  sessionId: string,
  assistantId?: string,
  options?: {
    thinking?: string;
    thinkingSignature?: string;
    tool_calls?: import('@/models/chat').ToolCall[];
    isStreaming?: boolean;
  },
): Message => ({
  id: createId(),
  content,
  role: 'assistant',
  sessionId,
  assistantId,
  ...options,
});

/**
 * 도구 실행 결과 메시지를 생성하는 헬퍼 함수
 * AI 서비스별 제약사항을 고려한 안전한 tool 메시지 생성
 */
export const createToolMessage = (
  content: MCPContent[],
  toolCallId: string, // ✅ tool_call_id 필수 파라미터
  sessionId: string,
  assistantId?: string,
): Message => {
  if (!toolCallId) {
    throw new Error('tool_call_id is required for tool messages');
  }

  return {
    id: createId(),
    content,
    role: 'tool',
    tool_call_id: toolCallId, // ✅ AI 서비스 제약 준수
    sessionId,
    assistantId,
  };
};

/**
 * Tool 실행 성공 메시지 생성 (tool_call_id 포함)
 */
export const createToolSuccessMessage = (
  result: string,
  toolCallId: string,
  sessionId: string,
  assistantId?: string,
): Message =>
  createToolMessage(
    stringToMCPContentArray(`✅ ${result}`),
    toolCallId,
    sessionId,
    assistantId,
  );

/**
 * Tool call과 tool result 메시지 쌍을 생성하는 헬퍼
 * 새로운 원자적 tool chain 패턴에 맞춤
 */
export const createToolMessagePair = (
  toolName: string,
  params: Record<string, unknown>,
  result: MCPContent[],
  toolCallId: string,
  sessionId: string,
  assistantId?: string,
): [Message, Message] => {
  const toolCallMessage: Message = {
    id: createId(),
    content: [], // Tool call은 content가 비어있을 수 있음
    role: 'assistant',
    tool_calls: [
      {
        id: toolCallId,
        type: 'function',
        function: {
          name: toolName,
          arguments: JSON.stringify(params),
        },
      },
    ],
    sessionId,
    assistantId,
  };

  const toolResultMessage: Message = {
    id: createId(),
    content: result,
    role: 'tool',
    tool_call_id: toolCallId,
    sessionId,
    assistantId,
  };

  return [toolCallMessage, toolResultMessage];
};
