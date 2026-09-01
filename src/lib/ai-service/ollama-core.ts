/**
 * Stable facade for Ollama-specific pure helpers.
 *
 * The implementation is split by responsibility so callers can keep importing
 * from `./ollama-core` without caring where message conversion, chunk
 * processing, or model capability helpers live.
 */

export {
  consoleLogger,
  noopLogger,
  type Logger,
  type OllamaToolCallAccumulator,
  type ProcessedChunk,
  type SimpleOllamaMessage,
  type ThinkTagFeedResult,
  type ThinkTagStreamMode,
  type ThinkTagStreamState,
} from './ollama-core-types';
export {
  convertAssistantMessage,
  convertMCPToolsToOllamaTools,
  convertMessage,
  convertToOllamaMessages,
  convertUserMessage,
  processMessageContent,
} from './ollama-message-converter';
export { processChunk } from './ollama-chunk-processor';
export { getModelToolSupport } from './ollama-model-utils';
export {
  createThinkTagStreamState,
  feedThinkTagDelta,
  flushThinkTagStream,
} from './think-tag-stream-parser';
