/**
 * @file Web Worker MCP Server Interface
 * @description Interface for MCP servers running in Web Workers
 */

import type { MCPTool } from '../protocol';
import type { MCPResponse, SamplingResponse } from '../protocol';
import type { SamplingOptions } from '../protocol';
import type { ServiceContext, ServiceContextOptions } from '@/features/tools';

/**
 * Defines the interface for an MCP server running in a Web Worker.
 */
export interface WebMCPServer {
  /** The name of the server. */
  name: string;
  /** An optional description of the server. */
  description?: string;
  /** The version of the server. */
  version?: string;
  /** An array of tools provided by the server. */
  tools: MCPTool[];
  /**
   * A function to call a tool on the server.
   * @param name The name of the tool to call.
   * @param args The arguments for the tool.
   * @returns A promise that resolves to an MCP response.
   */
  callTool: (name: string, args: unknown) => Promise<MCPResponse<unknown>>;
  /**
   * An optional function to perform text sampling.
   * @param prompt The prompt to use for sampling.
   * @param options Optional sampling parameters.
   * @returns A promise that resolves to a sampling response.
   */
  sampleText?: (
    prompt: string,
    options?: SamplingOptions,
  ) => Promise<SamplingResponse>;
  /** An optional function to get the service context. */
  getServiceContext?: (
    options?: ServiceContextOptions,
  ) => Promise<ServiceContext<unknown>>;
  /**
   * An optional function to switch the context for the server.
   * This allows servers to maintain state based on external context like session IDs or assistant IDs.
   * @param context The context object containing session/assistant identifiers and other state.
   * @returns A promise that resolves when the context is switched.
   */
  switchContext?: (context: ServiceContextOptions) => Promise<void>;
}

/**
 * Represents the state of a Web Worker MCP server.
 */
export interface WebMCPServerState {
  /** Indicates if the server's tools are loaded. */
  loaded: boolean;
  /** The list of tools provided by the server. */
  tools: MCPTool[];
  /** The last error message received from the server. */
  lastError?: string;
  /** The timestamp of the last activity from the server. */
  lastActivity?: number;
}
