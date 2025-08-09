/**
 * 🌐 Web Worker MCP Proxy
 *
 * Provides a clean interface for communicating with MCP servers running in web workers.
 * Handles message passing, error handling, and timeout management.
 */

import { createId } from '@paralleldrive/cuid2';
import {
  WebMCPMessage,
  WebMCPResponse,
  WebMCPProxyConfig,
  MCPTool,
} from '../mcp-types';
import { getLogger } from '../logger';

const logger = getLogger('WebMCPProxy');

export class WebMCPProxy {
  private worker: Worker | null = null;
  private pendingRequests = new Map<
    string,
    {
      resolve: (value: unknown) => void;
      reject: (error: Error) => void;
      timeout: ReturnType<typeof setTimeout>;
    }
  >();
  private config: Required<WebMCPProxyConfig>;
  private isInitialized = false;

  constructor(config: WebMCPProxyConfig) {
    this.config = {
      timeout: 30000, // 30 seconds default
      maxRetries: 3,
      ...config,
    };
  }

  /**
   * Initialize the worker and set up message handling
   */
  async initialize(): Promise<void> {
    if (this.isInitialized) {
      return;
    }

    try {
      logger.debug('Initializing WebMCP proxy', { config: this.config });

      this.worker = new Worker(this.config.workerPath, { type: 'module' });

      this.worker.onmessage = this.handleWorkerMessage.bind(this);
      this.worker.onerror = this.handleWorkerError.bind(this);

      // Test worker responsiveness
      await this.ping();

      this.isInitialized = true;
      logger.info('WebMCP proxy initialized successfully');
    } catch (error) {
      logger.error('Failed to initialize WebMCP proxy', error);
      this.cleanup();
      throw new Error(
        `Failed to initialize WebMCP proxy: ${error instanceof Error ? error.message : String(error)}`,
      );
    }
  }

  /**
   * Clean up resources
   */
  cleanup(): void {
    logger.debug('Cleaning up WebMCP proxy');

    // Reject all pending requests
    for (const [, { reject, timeout }] of this.pendingRequests.entries()) {
      clearTimeout(timeout);
      reject(new Error('Worker terminated'));
    }
    this.pendingRequests.clear();

    // Terminate worker
    if (this.worker) {
      this.worker.terminate();
      this.worker = null;
    }

    this.isInitialized = false;
  }

  /**
   * Send a message to the worker and wait for response
   */
  private async sendMessage<T = unknown>(
    message: Omit<WebMCPMessage, 'id'>,
  ): Promise<T> {
    if (!this.worker || !this.isInitialized) {
      throw new Error('WebMCP proxy not initialized');
    }

    const id = createId();
    const fullMessage: WebMCPMessage = { ...message, id };

    return new Promise<T>((resolve, reject) => {
      const timeout = setTimeout(() => {
        this.pendingRequests.delete(id);
        reject(new Error(`Request timeout after ${this.config.timeout}ms`));
      }, this.config.timeout);

      this.pendingRequests.set(id, {
        resolve: resolve as (value: unknown) => void,
        reject,
        timeout,
      });

      logger.debug('Sending message to worker', { message: fullMessage });
      this.worker!.postMessage(fullMessage);
    });
  }

  /**
   * Handle incoming messages from worker
   */
  private handleWorkerMessage(event: MessageEvent<WebMCPResponse>): void {
    const response = event.data;
    logger.debug('Received message from worker', { response });

    const pending = this.pendingRequests.get(response.id);
    if (!pending) {
      logger.warn('Received response for unknown request', {
        responseId: response.id,
      });
      return;
    }

    // Clean up
    clearTimeout(pending.timeout);
    this.pendingRequests.delete(response.id);

    // Handle response
    if (response.error) {
      pending.reject(new Error(response.error));
    } else {
      pending.resolve(response.result);
    }
  }

  /**
   * Handle worker errors
   */
  private handleWorkerError(error: ErrorEvent): void {
    logger.error('Worker error occurred', {
      message: error.message,
      filename: error.filename,
      lineno: error.lineno,
    });

    // Reject all pending requests
    for (const [, { reject, timeout }] of this.pendingRequests.entries()) {
      clearTimeout(timeout);
      reject(new Error(`Worker error: ${error.message}`));
    }
    this.pendingRequests.clear();
  }

  /**
   * Test worker responsiveness
   */
  async ping(): Promise<string> {
    const result = await this.sendMessage<string>({ type: 'ping' });
    return result;
  }

  /**
   * Load an MCP server module in the worker
   */
  async loadServer(serverName: string): Promise<{
    name: string;
    description?: string;
    version?: string;
    toolCount: number;
  }> {
    const result = await this.sendMessage<{
      name: string;
      description?: string;
      version?: string;
      toolCount: number;
    }>({
      type: 'loadServer',
      serverName,
    });

    logger.info('MCP server loaded', { serverName, result });
    return result;
  }

  /**
   * List all available tools from loaded servers
   */
  async listAllTools(): Promise<MCPTool[]> {
    const tools = await this.sendMessage<MCPTool[]>({ type: 'listTools' });
    logger.debug('Listed all tools', { toolCount: tools.length });
    return tools;
  }

  /**
   * List tools from a specific server
   */
  async listTools(serverName: string): Promise<MCPTool[]> {
    const tools = await this.sendMessage<MCPTool[]>({
      type: 'listTools',
      serverName,
    });

    logger.debug('Listed tools for server', {
      serverName,
      toolCount: tools.length,
    });
    return tools;
  }

  /**
   * Call a tool on a specific server
   */
  async callTool(
    serverName: string,
    toolName: string,
    args: unknown,
  ): Promise<unknown> {
    logger.debug('Calling tool', { serverName, toolName, args });

    const result = await this.sendMessage({
      type: 'callTool',
      serverName,
      toolName,
      args,
    });

    logger.debug('Tool call completed', { serverName, toolName, result });
    return result;
  }

  /**
   * Get proxy status
   */
  getStatus(): {
    initialized: boolean;
    pendingRequests: number;
    workerPath: string;
  } {
    return {
      initialized: this.isInitialized,
      pendingRequests: this.pendingRequests.size,
      workerPath: this.config.workerPath,
    };
  }
}

/**
 * Factory function for creating WebMCP proxy instances
 */
export function createWebMCPProxy(config: WebMCPProxyConfig): WebMCPProxy {
  return new WebMCPProxy(config);
}
