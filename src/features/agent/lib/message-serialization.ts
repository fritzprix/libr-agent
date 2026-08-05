import type { Message, ToolCall } from '@/models/chat';
import type { MCPContent, MCPTextContent } from '@/lib/mcp';
import { extractTextContent } from '@/lib/message-utils';
import { messageToMarkdown } from '@/lib/message-markdown';
import { hasToolCallError } from '@/lib/tool-call-utils';

export type MessageSerializationMode = 'full' | 'text' | 'tools';

export interface SerializeMessageOptions {
  mode?: MessageSerializationMode;
  includeThinking?: boolean;
  includeToolCalls?: boolean;
  includeAttachments?: boolean;
  includeToolResults?: boolean;
  /** Precomputed interleaved content (from computeDisplayContent) */
  displayContent?: MCPContent[];
  toolResultsMap?: Map<string, Message>;
}

/**
 * Clipboard/export shape aligned with MCP CallToolResult:
 * top-level `isError`, content items without item-level `isError`, no redundant `text`.
 */
export interface SerializedToolCall {
  id: string;
  name: string;
  arguments: unknown;
  result?: {
    content: MCPContent[];
    isError?: boolean;
  };
}

/**
 * Strip non-standard item-level `isError` from text content for MCP-compliant export.
 */
function toCallToolResultContent(content: MCPContent[]): MCPContent[] {
  return content.map((item): MCPContent => {
    if (item.type !== 'text') {
      return item;
    }

    const cleaned: MCPTextContent = {
      type: 'text',
      text: item.text,
    };
    if (item.annotations !== undefined) {
      cleaned.annotations = item.annotations;
    }
    if (item.serviceInfo !== undefined) {
      cleaned.serviceInfo = item.serviceInfo;
    }
    return cleaned;
  });
}

function parseToolArguments(argumentsJson: string): unknown {
  try {
    return JSON.parse(argumentsJson) as unknown;
  } catch {
    return argumentsJson;
  }
}

function collectToolCalls(message: Message, content: MCPContent[]): ToolCall[] {
  const fromMessage = message.tool_calls ?? [];
  if (fromMessage.length > 0) {
    return fromMessage;
  }

  return content.flatMap((item): ToolCall[] => {
    if (item.type !== 'tool_call') {
      return [];
    }
    return [
      {
        id: item.id,
        type: 'function',
        function: {
          name: item.name,
          arguments: item.arguments,
        },
      },
    ];
  });
}

function resolveToolResult(
  toolCallId: string,
  occurrenceIndex: number,
  toolResultsMap?: Map<string, Message>,
): Message | undefined {
  if (!toolResultsMap) {
    return undefined;
  }
  const key =
    occurrenceIndex === 0 ? toolCallId : `${toolCallId}_dup${occurrenceIndex}`;
  return toolResultsMap.get(key) ?? toolResultsMap.get(toolCallId);
}

function buildEffectiveMessage(
  message: Message,
  displayContent?: MCPContent[],
): Message {
  if (!displayContent) {
    return message;
  }

  // displayContent already interleaves tool_call items; avoid duplicating
  // message.tool_calls in markdown output.
  return {
    ...message,
    content: displayContent,
    tool_calls: undefined,
  };
}

function formatToolResultsMarkdown(
  toolCalls: ToolCall[],
  toolResultsMap?: Map<string, Message>,
): string {
  if (!toolResultsMap || toolCalls.length === 0) {
    return '';
  }

  const idUsageCount = new Map<string, number>();
  const parts: string[] = [];

  for (const toolCall of toolCalls) {
    const occurrence = idUsageCount.get(toolCall.id) ?? 0;
    idUsageCount.set(toolCall.id, occurrence + 1);

    const result = resolveToolResult(toolCall.id, occurrence, toolResultsMap);
    if (!result) {
      continue;
    }

    const resultText = extractTextContent(result).trim();
    const errorNote = result.error?.displayMessage
      ? `\n\n**Error:** ${result.error.displayMessage}`
      : '';

    parts.push(
      `### Tool Result: ${toolCall.function.name}\n\n${resultText || '_(empty)_'}${errorNote}`,
    );
  }

  return parts.join('\n\n');
}

/**
 * Serialize a chat bubble message for clipboard copy.
 */
export function serializeMessageForClipboard(
  message: Message,
  options: SerializeMessageOptions = {},
): string {
  const mode = options.mode ?? 'full';

  if (mode === 'text') {
    return serializeMessageTextOnly(message, options.displayContent);
  }

  if (mode === 'tools') {
    return serializeToolCallsForClipboard(
      collectToolCalls(
        message,
        options.displayContent ?? message.content ?? [],
      ),
      options.toolResultsMap,
    );
  }

  const effective = buildEffectiveMessage(message, options.displayContent);
  const markdown = messageToMarkdown(effective, {
    includeThinking: options.includeThinking ?? true,
    includeToolCalls: options.includeToolCalls ?? true,
    includeTimestamps: false,
    includeSystem: true,
  });

  const toolCalls = collectToolCalls(
    message,
    options.displayContent ?? message.content ?? [],
  );
  const resultsMarkdown =
    options.includeToolResults === false
      ? ''
      : formatToolResultsMarkdown(toolCalls, options.toolResultsMap);

  if (!resultsMarkdown) {
    return markdown;
  }

  return markdown
    ? `${markdown}\n\n---\n\n${resultsMarkdown}`
    : resultsMarkdown;
}

/**
 * Serialize message body for file download (MD/PDF).
 * Returns the reply text only — no role header, thinking, or tool calls.
 */
export function serializeMessageForDownload(
  message: Message,
  options: Pick<SerializeMessageOptions, 'displayContent'> = {},
): string {
  return serializeMessageTextOnly(message, options.displayContent);
}

/**
 * Extract plain text content only (no thinking, tools, or attachments).
 */
export function serializeMessageTextOnly(
  message: Message,
  displayContent?: MCPContent[],
): string {
  const content = displayContent ?? message.content ?? [];
  const textFromContent = content
    .filter(
      (item): item is Extract<MCPContent, { type: 'text' }> =>
        item.type === 'text' && typeof item.text === 'string',
    )
    .map((item) => item.text)
    .join('\n\n')
    .trim();

  if (textFromContent) {
    return textFromContent;
  }

  return extractTextContent(message).trim();
}

/**
 * Serialize tool calls (and optional results) as formatted JSON.
 */
export function serializeToolCallsForClipboard(
  toolCalls: ToolCall[],
  toolResultsMap?: Map<string, Message>,
): string {
  if (toolCalls.length === 0) {
    return '[]';
  }

  const idUsageCount = new Map<string, number>();
  const payload: SerializedToolCall[] = toolCalls.map((toolCall) => {
    const occurrence = idUsageCount.get(toolCall.id) ?? 0;
    idUsageCount.set(toolCall.id, occurrence + 1);

    const resultMessage = resolveToolResult(
      toolCall.id,
      occurrence,
      toolResultsMap,
    );

    const entry: SerializedToolCall = {
      id: toolCall.id,
      name: toolCall.function.name,
      arguments: parseToolArguments(toolCall.function.arguments),
    };

    if (resultMessage) {
      const isError = hasToolCallError(resultMessage);
      entry.result = {
        content: toCallToolResultContent(resultMessage.content ?? []),
        ...(isError ? { isError: true } : {}),
      };
    }

    return entry;
  });

  return JSON.stringify(payload, null, 2);
}

export function buildMessageExportFilename(
  _message: Message,
  extension: 'md' | 'pdf',
): string {
  return `message.${extension}`;
}
