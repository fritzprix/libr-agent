import type {
  MCPContent,
  MCPTextContent,
  MCPThinkingContent,
  MCPToolCallContent,
} from '@/lib/mcp-types';
import type { ToolCall } from '@/models/chat';
import type { TokenUsage } from '@/lib/ai-service/types';

export class StreamAccumulator {
  content: MCPContent[] = [];
  activeToolCallIndices = new Map<number, number>();
  thinkingStartTime: number | undefined;
  firstChunkTime: number | undefined;
  finalUsage: TokenUsage | undefined;
  startTime: number;

  constructor() {
    this.startTime = performance.now();
  }

  processChunk(chunk: string): void {
    if (this.firstChunkTime === undefined) {
      this.firstChunkTime = performance.now();
    }

    let parsedChunk: Record<string, unknown>;
    try {
      parsedChunk = JSON.parse(chunk);
    } catch {
      parsedChunk = { content: chunk };
    }

    // 1. Accumulate Content (Text)
    if (parsedChunk.content && typeof parsedChunk.content === 'string') {
      const lastItem = this.content[this.content.length - 1];
      if (lastItem && lastItem.type === 'text') {
        (lastItem as MCPTextContent).text += parsedChunk.content;
      } else {
        this.content.push({ type: 'text', text: parsedChunk.content });
      }
    }

    // 2. Accumulate Thinking
    if (parsedChunk.thinking && typeof parsedChunk.thinking === 'string') {
      if (this.thinkingStartTime === undefined) {
        this.thinkingStartTime = performance.now();
      }

      const lastItem = this.content[this.content.length - 1];
      if (lastItem && lastItem.type === 'thinking') {
        (lastItem as MCPThinkingContent).thinking += parsedChunk.thinking;
      } else {
        this.content.push({
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
            this.content.push({
              type: 'tool_call',
              id: toolCallChunk.id || '',
              name: toolCallChunk.function?.name || '',
              arguments: toolCallChunk.function?.arguments || '',
            });
            return;
          }

          // Case B: Index defined (Incremental streaming)
          if (this.activeToolCallIndices.has(index)) {
            const contentIndex = this.activeToolCallIndices.get(index)!;
            const targetBlock = this.content[contentIndex] as MCPToolCallContent;

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
            this.content.push(newBlock);
            this.activeToolCallIndices.set(index, this.content.length - 1);
          }
        },
      );
    }

    // Accumulate usage metrics
    if (parsedChunk.usage) {
      const incomingUsage = parsedChunk.usage as TokenUsage;
      if (this.finalUsage) {
        this.finalUsage = {
          promptTokens:
            incomingUsage.promptTokens || this.finalUsage.promptTokens,
          completionTokens:
            incomingUsage.completionTokens || this.finalUsage.completionTokens,
          totalTokens: incomingUsage.totalTokens || this.finalUsage.totalTokens,
          details: {
            ...this.finalUsage.details,
            ...incomingUsage.details,
          },
        };
      } else {
        this.finalUsage = incomingUsage;
      }
    }
  }

  getCurrentThinkingTime(): number | undefined {
    if (this.thinkingStartTime !== undefined) {
      return (performance.now() - this.thinkingStartTime) / 1000;
    }
    return undefined;
  }

  getLegacyToolCalls(): ToolCall[] {
    return this.content
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
  }

  getLegacyThinking(): string {
    return this.content
      .filter((c) => c.type === 'thinking')
      .map((c) => (c as MCPThinkingContent).thinking)
      .join('\n');
  }

  finalizeUsage(endTime: number) {
    const totalDurationMs = endTime - this.startTime;

    if (this.finalUsage && this.finalUsage.completionTokens > 0) {
      if (!this.finalUsage.details) {
        this.finalUsage.details = {};
      }
      // If provider didn't give duration, use calculated timings
      if (!this.finalUsage.details.evalDuration) {
        if (this.firstChunkTime) {
          this.finalUsage.details.promptEvalDuration =
            this.firstChunkTime - this.startTime;
          this.finalUsage.details.evalDuration = endTime - this.firstChunkTime;
        } else {
          this.finalUsage.details.evalDuration = totalDurationMs;
        }
      }
      // If timeToFirstToken wasn't provided by the service, calculate it
      if (!this.finalUsage.details.timeToFirstToken && this.firstChunkTime) {
        this.finalUsage.details.timeToFirstToken =
          this.firstChunkTime - this.startTime;
      }
    }
    return this.finalUsage;
  }
}
