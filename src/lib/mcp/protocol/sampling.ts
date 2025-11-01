/**
 * @file MCP Sampling Types
 * @description Text generation and sampling request/response types
 */

import type { MCPResponse, SamplingResult } from './response';

/**
 * Defines options for text generation (sampling).
 */
export interface SamplingOptions {
  model?: string;
  maxTokens?: number;
  temperature?: number;
  topP?: number;
  topK?: number;
  stopSequences?: string[];
  presencePenalty?: number;
  frequencyPenalty?: number;
}

/**
 * Represents a request for text generation.
 */
export interface SamplingRequest {
  prompt: string;
  options?: SamplingOptions;
}

/**
 * Represents a response to a sampling request.
 */
export interface SamplingResponse extends MCPResponse<unknown> {
  result?: SamplingResult;
}
