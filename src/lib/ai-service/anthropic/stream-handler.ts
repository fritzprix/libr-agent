import { getLogger } from '../../logger';
import { formatToolCall } from '../utils';
import { MAX_PARTIAL_TOOL_INPUT_LENGTH } from './constants';
import { ToolCallAccumulator } from './types';

const logger = getLogger('AnthropicStreamHandler');

export interface ToolCallYield {
  tool_calls: unknown[];
}

export class ToolCallStreamAccumulator {
  private accumulators = new Map<number, ToolCallAccumulator>();

  handleContentBlockStart(chunk: any): void {
    if (chunk.content_block.type === 'tool_use') {
      const initialInput =
        chunk.content_block.input &&
        typeof chunk.content_block.input === 'object' &&
        !Array.isArray(chunk.content_block.input)
          ? (chunk.content_block.input as Record<string, unknown>)
          : null;
      this.accumulators.set(chunk.index, {
        id: chunk.content_block.id,
        name: chunk.content_block.name,
        partialJson: '',
        index: chunk.index,
        yielded: false, // Initial value is false
        initialInput,
      });
      logger.debug('Started tool call accumulation', {
        index: chunk.index,
        id: chunk.content_block.id,
        name: chunk.content_block.name,
      });
    }
  }

  handleInputJsonDelta(chunk: any): ToolCallYield | null {
    const accumulator = this.accumulators.get(chunk.index);
    if (!accumulator) return null;

    // log the incoming partial fragment for inspection
    logger.info('Anthropic input_json_delta fragment', {
      index: chunk.index,
      fragment: chunk.delta.partial_json,
      currentLength: accumulator.partialJson.length,
    });
    accumulator.partialJson += chunk.delta.partial_json;
    if (accumulator.partialJson.length > MAX_PARTIAL_TOOL_INPUT_LENGTH) {
      logger.error('Tool call input exceeded maximum buffered length', {
        index: chunk.index,
        toolId: accumulator.id,
        name: accumulator.name,
      });
      this.accumulators.delete(chunk.index);
      accumulator.yielded = true;
      return null;
    }
    logger.debug('Accumulated partial JSON', {
      index: chunk.index,
      partialJson: accumulator.partialJson,
    });

    // Try to parse the accumulated JSON only if not already yielded
    if (!accumulator.yielded) {
      const trimmedPartial = accumulator.partialJson.trim();
      if (trimmedPartial.length === 0) {
        logger.debug('No complete JSON fragment yet; waiting', {
          index: chunk.index,
          id: accumulator.id,
        });
        return null;
      }
      try {
        const parsedInput = JSON.parse(trimmedPartial) as Record<
          string,
          unknown
        >;
        // If parsing succeeds, yield the tool call and mark as yielded
        accumulator.yielded = true; // Prevent duplicate yields
        logger.debug('Tool call yielded successfully', {
          index: chunk.index,
          id: accumulator.id,
          name: accumulator.name,
        });
        return {
          tool_calls: [
            formatToolCall(accumulator.id, accumulator.name, parsedInput),
          ],
        };
      } catch (parseError) {
        // Continue accumulating if JSON is still incomplete
        logger.debug('JSON still incomplete, continuing accumulation', {
          error: parseError,
          partialJson: accumulator.partialJson,
        });
      }
    }
    return null;
  }

  handleContentBlockStop(chunk: any): ToolCallYield | null {
    logger.info('Anthropic content_block_stop', { index: chunk.index });
    // Final attempt to parse accumulated JSON only if not already yielded
    const accumulator = this.accumulators.get(chunk.index);
    let result: ToolCallYield | null = null;

    if (accumulator && accumulator.partialJson && !accumulator.yielded) {
      const trimmedPartial = accumulator.partialJson.trim();
      try {
        const parsedInput = JSON.parse(trimmedPartial) as Record<
          string,
          unknown
        >;
        logger.info('Tool call completed on content_block_stop', {
          id: accumulator.id,
          name: accumulator.name,
          input: parsedInput,
        });
        // Final tool call yield if not already done
        result = {
          tool_calls: [
            formatToolCall(accumulator.id, accumulator.name, parsedInput),
          ],
        };
        accumulator.yielded = true;
      } catch (parseError) {
        if (accumulator.initialInput) {
          logger.info('Using initial tool input from content_block_start', {
            id: accumulator.id,
            name: accumulator.name,
          });
          result = {
            tool_calls: [
              formatToolCall(
                accumulator.id,
                accumulator.name,
                accumulator.initialInput,
              ),
            ],
          };
          accumulator.yielded = true;
        } else {
          logger.error('Failed to parse final tool call JSON', {
            error: parseError,
            partialJson: accumulator.partialJson,
            toolId: accumulator.id,
            toolName: accumulator.name,
          });
        }
      }
    } else if (
      accumulator &&
      !accumulator.yielded &&
      accumulator.initialInput
    ) {
      logger.info('Tool call completed using initial input without deltas', {
        id: accumulator.id,
        name: accumulator.name,
      });
      result = {
        tool_calls: [
          formatToolCall(
            accumulator.id,
            accumulator.name,
            accumulator.initialInput,
          ),
        ],
      };
      accumulator.yielded = true;
    }

    // Clean up accumulator regardless of yield status
    if (accumulator) {
      this.accumulators.delete(chunk.index);
      logger.debug('Cleaned up tool call accumulator', {
        index: chunk.index,
        id: accumulator.id,
        wasYielded: accumulator.yielded,
      });
    }

    return result;
  }
}
