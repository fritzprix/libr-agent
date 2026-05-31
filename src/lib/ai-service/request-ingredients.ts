import type { MCPTool } from '@/lib/mcp';
import type { Message } from '@/models/chat';

export interface MessageIngredientSummary {
  messageCount: number;
  roleCounts: Record<string, number>;
  sourceCounts: Record<string, number>;
  compactSummaryCount: number;
  compactionInstructionCount: number;
  sessionContextCount: number;
  externalRequestCount: number;
  assistantToolCallCount: number;
}

export interface CompactionRequestSizeSummary extends MessageIngredientSummary {
  contentPartCount: number;
  contentTypeCounts: Record<string, number>;
  textChars: number;
  thinkingChars: number;
  contentToolCallArgumentChars: number;
  assistantToolCallArgumentChars: number;
  attachmentCount: number;
  totalMessagePayloadChars: number;
  averageMessagePayloadChars: number;
  maxMessagePayloadChars: number;
  maxMessageContentParts: number;
  systemPromptLength: number;
  toolsCount: number;
  toolsJsonChars: number;
  compactionInstruction: {
    included: boolean;
    placement: 'messages[last-user]' | 'missing';
    messageIndex: number;
    contentPartIndex: number;
    role: 'user' | 'missing';
    source: string;
    textChars: number;
    preview: string;
  };
}

export interface RequestIngredientMessageLike {
  role: string;
  source?: string | null;
  tool_calls?: ReadonlyArray<unknown> | null;
}

export function summarizeMessageIngredients(
  messages: ReadonlyArray<RequestIngredientMessageLike>,
): MessageIngredientSummary {
  const roleCounts: Record<string, number> = {};
  const sourceCounts: Record<string, number> = {};

  let compactSummaryCount = 0;
  let compactionInstructionCount = 0;
  let sessionContextCount = 0;
  let externalRequestCount = 0;
  let assistantToolCallCount = 0;

  for (const message of messages) {
    roleCounts[message.role] = (roleCounts[message.role] ?? 0) + 1;

    const source = String(message.source ?? 'none');
    sourceCounts[source] = (sourceCounts[source] ?? 0) + 1;

    if (source === 'compact-summary') {
      compactSummaryCount += 1;
    }
    if (source === 'compaction-instruction') {
      compactionInstructionCount += 1;
    }
    if (source === 'session-context') {
      sessionContextCount += 1;
    }
    if (
      source === 'ui' ||
      source === 'api' ||
      source === 'channel' ||
      source === 'scheduled_task'
    ) {
      externalRequestCount += 1;
    }
    if (message.tool_calls?.length) {
      assistantToolCallCount += 1;
    }
  }

  return {
    messageCount: messages.length,
    roleCounts,
    sourceCounts,
    compactSummaryCount,
    compactionInstructionCount,
    sessionContextCount,
    externalRequestCount,
    assistantToolCallCount,
  };
}

export function summarizeCompactionRequestSizes(args: {
  messages: ReadonlyArray<Message>;
  systemPrompt?: string;
  availableTools?: ReadonlyArray<MCPTool>;
}): CompactionRequestSizeSummary {
  const ingredientSummary = summarizeMessageIngredients(args.messages);
  const contentTypeCounts: Record<string, number> = {};

  let contentPartCount = 0;
  let textChars = 0;
  let thinkingChars = 0;
  let contentToolCallArgumentChars = 0;
  let assistantToolCallArgumentChars = 0;
  let attachmentCount = 0;
  let totalMessagePayloadChars = 0;
  let maxMessagePayloadChars = 0;
  let maxMessageContentParts = 0;
  let compactionInstructionMessageIndex = -1;
  let compactionInstructionContentPartIndex = -1;
  let compactionInstructionText = '';

  for (const [messageIndex, message] of args.messages.entries()) {
    let messagePayloadChars = 0;
    const messageContentParts = message.content.length;

    for (const [contentPartIndex, part] of message.content.entries()) {
      contentPartCount += 1;
      contentTypeCounts[part.type] = (contentTypeCounts[part.type] ?? 0) + 1;

      if (
        message.source === 'compaction-instruction' &&
        part.type === 'text' &&
        compactionInstructionMessageIndex === -1
      ) {
        compactionInstructionMessageIndex = messageIndex;
        compactionInstructionContentPartIndex = contentPartIndex;
        compactionInstructionText = part.text;
      }

      switch (part.type) {
        case 'text':
          textChars += part.text.length;
          messagePayloadChars += part.text.length;
          break;
        case 'thinking':
          thinkingChars += part.thinking.length;
          messagePayloadChars += part.thinking.length;
          break;
        case 'tool_call':
          contentToolCallArgumentChars += part.arguments.length;
          messagePayloadChars += part.arguments.length;
          break;
        default:
          break;
      }
    }

    for (const toolCall of message.tool_calls ?? []) {
      assistantToolCallArgumentChars += toolCall.function.arguments.length;
      messagePayloadChars += toolCall.function.arguments.length;
    }

    attachmentCount += message.attachments?.length ?? 0;
    totalMessagePayloadChars += messagePayloadChars;
    maxMessagePayloadChars = Math.max(
      maxMessagePayloadChars,
      messagePayloadChars,
    );
    maxMessageContentParts = Math.max(
      maxMessageContentParts,
      messageContentParts,
    );
  }

  const toolsJson = JSON.stringify(args.availableTools ?? []);

  return {
    ...ingredientSummary,
    contentPartCount,
    contentTypeCounts,
    textChars,
    thinkingChars,
    contentToolCallArgumentChars,
    assistantToolCallArgumentChars,
    attachmentCount,
    totalMessagePayloadChars,
    averageMessagePayloadChars:
      args.messages.length > 0
        ? totalMessagePayloadChars / args.messages.length
        : 0,
    maxMessagePayloadChars,
    maxMessageContentParts,
    systemPromptLength: args.systemPrompt?.length ?? 0,
    toolsCount: args.availableTools?.length ?? 0,
    toolsJsonChars: toolsJson.length,
    compactionInstruction: {
      included: compactionInstructionMessageIndex !== -1,
      placement:
        compactionInstructionMessageIndex !== -1
          ? 'messages[last-user]'
          : 'missing',
      messageIndex: compactionInstructionMessageIndex,
      contentPartIndex: compactionInstructionContentPartIndex,
      role: compactionInstructionMessageIndex !== -1 ? 'user' : 'missing',
      source:
        compactionInstructionMessageIndex !== -1
          ? 'compaction-instruction'
          : 'missing',
      textChars: compactionInstructionText.length,
      preview:
        compactionInstructionText.length > 160
          ? `${compactionInstructionText.slice(0, 160)}…`
          : compactionInstructionText,
    },
  };
}
