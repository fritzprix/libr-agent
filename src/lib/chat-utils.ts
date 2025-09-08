import { createId } from '@paralleldrive/cuid2';
import { Message } from '@/models/chat';
import { stringToMCPContentArray } from '@/lib/utils';

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
 * 도구 실행 결과 메시지를 생성하는 헬퍼 함수
 * AI 서비스별 제약사항을 고려한 안전한 tool 메시지 생성
 */
export const createToolMessage = (
  content: ReturnType<typeof stringToMCPContentArray>,
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
  result: string | ReturnType<typeof stringToMCPContentArray>,
  toolCallId: string,
  sessionId: string,
  assistantId?: string,
  isError = false,
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

  const content =
    typeof result === 'string'
      ? stringToMCPContentArray(isError ? `❌ ${result}` : `✅ ${result}`)
      : result;

  const toolResultMessage: Message = {
    id: createId(),
    content,
    role: 'tool',
    tool_call_id: toolCallId,
    sessionId,
    assistantId,
  };

  return [toolCallMessage, toolResultMessage];
};
