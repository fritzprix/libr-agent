import {
  MCPContent,
  MCPTextContent,
  MCPThinkingContent,
  MCPToolCallContent,
} from '@/lib/mcp-types';
import { ToolCall, Message } from '@/models/chat';
import { TokenUsage } from '@/lib/ai-service/types';
import { getLogger } from '@/lib/logger';

const logger = getLogger('StreamProcessor');

interface StreamProcessorCallbacks {
  onUpdate: (streamingMessage: Partial<Message>) => void;
  signal: AbortSignal;
}

export async function processLLMStream(
  sessionId: string,
  streamGenerator: AsyncGenerator<string>,
  initialMessage: Partial<Message>,
  callbacks: StreamProcessorCallbacks,
): Promise<{
  content: MCPContent[];
  finalUsage: TokenUsage | undefined;
  thinkingStartTime: number | undefined;
  firstChunkTime: number | undefined;
}> {
  const { onUpdate, signal } = callbacks;

  // Accumulate chunks
  const content: MCPContent[] = [];
  // Map chunk tool index -> content array index
  const activeToolCallIndices = new Map<number, number>();

  let thinkingStartTime: number | undefined;
  let finalUsage: TokenUsage | undefined;
  let firstChunkTime: number | undefined;

  for await (const chunk of streamGenerator) {
    // Capture Time to First Token (TTFT) for detailed metrics
    if (firstChunkTime === undefined) {
      firstChunkTime = performance.now();
    }

    // Check if aborted
    if (signal.aborted) {
      logger.warn('Completion request aborted', { sessionId });
      throw new Error('Request aborted');
    }

    // Parse chunk (it's a JSON string)
    let parsedChunk: Record<string, unknown>;
    try {
      parsedChunk = JSON.parse(chunk);
    } catch {
      // If parsing fails, treat it as plain text content
      parsedChunk = { content: chunk };
    }

    // 1. Accumulate Content (Text)
    if (parsedChunk.content && typeof parsedChunk.content === 'string') {
      const lastItem = content[content.length - 1];
      if (lastItem && lastItem.type === 'text') {
        (lastItem as MCPTextContent).text += parsedChunk.content;
      } else {
        content.push({ type: 'text', text: parsedChunk.content });
      }
    }

    // 2. Accumulate Thinking
    if (parsedChunk.thinking && typeof parsedChunk.thinking === 'string') {
      if (thinkingStartTime === undefined) {
        thinkingStartTime = performance.now();
      }

      const lastItem = content[content.length - 1];
      if (lastItem && lastItem.type === 'thinking') {
        (lastItem as MCPThinkingContent).thinking += parsedChunk.thinking;
      } else {
        content.push({
          type: 'thinking',
          thinking: parsedChunk.thinking,
        });
      }
    }

    // 3. Accumulate Tool Calls
    if (parsedChunk.tool_calls && Array.isArray(parsedChunk.tool_calls)) {
      (parsedChunk.tool_calls as (ToolCall & { index?: number })[]).forEach(
        (toolCallChunk) => {
          const { index } = toolCallChunk;

          // Case A: Index undefined (Complete tool call or single linear) -> Push new
          if (index === undefined) {
            content.push({
              type: 'tool_call',
              id: toolCallChunk.id || '',
              name: toolCallChunk.function?.name || '',
              arguments: toolCallChunk.function?.arguments || '',
            });
            return;
          }

          // Case B: Index defined (Incremental streaming)
          if (activeToolCallIndices.has(index)) {
            const contentIndex = activeToolCallIndices.get(index)!;
            const targetBlock = content[contentIndex] as MCPToolCallContent;

            // Merge
            if (toolCallChunk.id && !targetBlock.id) {
              targetBlock.id = toolCallChunk.id;
            }
            if (toolCallChunk.function?.name) {
              if (!targetBlock.name) {
                targetBlock.name = toolCallChunk.function.name;
              } else if (
                targetBlock.name !== toolCallChunk.function.name &&
                !toolCallChunk.function.name.startsWith(targetBlock.name)
              ) {
                targetBlock.name = toolCallChunk.function.name;
              }
            }
            // Arguments update
            if (toolCallChunk.function?.arguments) {
              targetBlock.arguments += toolCallChunk.function.arguments;
            }
          } else {
            // New tool call at this index
            const newBlock: MCPToolCallContent = {
              type: 'tool_call',
              id: toolCallChunk.id || '',
              name: toolCallChunk.function?.name || '',
              arguments: toolCallChunk.function?.arguments || '',
            };
            content.push(newBlock);
            activeToolCallIndices.set(index, content.length - 1);
          }
        },
      );
    }

    // Calculate Thinking Time (seconds)
    let currentThinkingTime: number | undefined;
    if (thinkingStartTime !== undefined) {
      currentThinkingTime = (performance.now() - thinkingStartTime) / 1000;
    }

    // Accumulate usage metrics
    if (parsedChunk.usage) {
      const incomingUsage = parsedChunk.usage as TokenUsage;
      if (finalUsage) {
        finalUsage = {
          promptTokens: incomingUsage.promptTokens || finalUsage.promptTokens,
          completionTokens:
            incomingUsage.completionTokens || finalUsage.completionTokens,
          totalTokens: incomingUsage.totalTokens || finalUsage.totalTokens,
          details: {
            ...finalUsage.details,
            ...incomingUsage.details,
          },
        };
      } else {
        finalUsage = incomingUsage;
      }
    }

    // Derive legacy fields from content
    const legacyToolCalls: ToolCall[] = content
      .filter((c) => c.type === 'tool_call')
      .map((c) => {
        const tc = c as MCPToolCallContent;
        return {
          id: tc.id,
          type: 'function',
          function: {
            name: tc.name,
            arguments: tc.arguments,
          },
        };
      });

    const legacyThinking = content
      .filter((c) => c.type === 'thinking')
      .map((c) => (c as MCPThinkingContent).thinking)
      .join('\n');

    onUpdate({
      ...initialMessage,
      content, // Unified content array
      tool_calls: legacyToolCalls.length > 0 ? legacyToolCalls : undefined,
      thinking: legacyThinking || undefined,
      thinkingTime: currentThinkingTime,
      usage: finalUsage,
      isStreaming: true,
    });
  }

  return {
    content,
    finalUsage,
    thinkingStartTime,
    firstChunkTime,
  };
}
