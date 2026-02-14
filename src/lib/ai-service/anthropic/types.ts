/**
 * Extended usage interface for Anthropic's response that includes prompt caching fields.
 * Anthropic SDK types may not include these yet, so we define them explicitly.
 * @internal
 */
export interface AnthropicUsageWithCache {
  input_tokens: number;
  output_tokens: number;
  cache_creation_input_tokens?: number;
  cache_read_input_tokens?: number;
}

/**
 * An internal helper interface to accumulate partial JSON data for a tool call
 * during a streaming response.
 * @internal
 */
export interface ToolCallAccumulator {
  id: string;
  name: string;
  partialJson: string;
  index: number;
  yielded: boolean; // Track if already yielded to prevent duplicates
  initialInput?: Record<string, unknown> | null;
}
