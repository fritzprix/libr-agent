/**
 * @file Web Worker MCP Proxy Configuration
 * @description Configuration types for Web Worker proxy
 */

/**
 * Defines the configuration for the Web Worker MCP proxy.
 */
export interface WebMCPProxyConfig {
  /** The path to the worker script. */
  workerPath?: string;
  /** An existing worker instance to use. */
  workerInstance?: Worker;
  /** The timeout for requests in milliseconds. */
  timeout?: number;
  /** Options for retrying failed requests. */
  retryOptions?: {
    maxRetries?: number;
    baseDelay?: number;
    maxDelay?: number;
    timeout?: number;
  };
}
