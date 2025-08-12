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
import { withTimeout, RetryOptions } from '../retry-utils';

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
  private config: WebMCPProxyConfig & {
    timeout: number;
    retryOptions: RetryOptions;
  };
  private isInitialized = false;
  private initializationPromise: Promise<void> | null = null;

  constructor(config: WebMCPProxyConfig) {
    this.config = {
      timeout: 30000, // 30 seconds default
      retryOptions: {
        maxRetries: 3,
        baseDelay: 1000,
        maxDelay: 10000,
        exponentialBackoff: true,
        ...config.retryOptions,
      },
      ...config,
    };

    logger.debug('WebMCPProxy created', {
      workerType: this.config.workerInstance ? 'instance' : 'path',
      timeout: this.config.timeout,
    });
  }

  /**
   * Initialize the worker and set up message handling
   */
  async initialize(): Promise<void> {
    if (this.isInitialized) {
      logger.debug('WebMCP proxy already initialized');
      return;
    }

    // Prevent multiple concurrent initializations
    if (this.initializationPromise) {
      return this.initializationPromise;
    }

    this.initializationPromise = this._doInitialize();
    try {
      await this.initializationPromise;
    } finally {
      this.initializationPromise = null;
    }
  }

  private async _doInitialize(): Promise<void> {
    try {
      logger.info('Initializing WebMCP proxy');

      // Create worker from instance or path
      if (this.config.workerInstance) {
        this.worker = this.config.workerInstance;
        logger.debug('Using provided worker instance');
      } else if (this.config.workerPath) {
        logger.debug('Creating worker from path');
        this.worker = new Worker(this.config.workerPath, { type: 'module' });
      } else {
        throw new Error('Either workerInstance or workerPath must be provided');
      }

      if (!this.worker) {
        throw new Error('Worker creation failed');
      }

      // Setup event handlers
      this.worker.onmessage = this.handleWorkerMessage.bind(this);
      this.worker.onerror = this.handleWorkerError.bind(this);

      // Test worker responsiveness with retry
      await this.ping();

      this.isInitialized = true;
      logger.info('WebMCP proxy initialized successfully');
    } catch (error) {
      const errorMessage =
        error instanceof Error ? error.message : String(error);
      logger.error('Failed to initialize WebMCP proxy', {
        error: errorMessage,
      });

      this.cleanup();
      throw new Error(`Failed to initialize WebMCP proxy: ${errorMessage}`);
    }
  }

  /**
   * Clean up resources
   */
  cleanup(): void {
    logger.debug('Cleaning up WebMCP proxy', {
      pendingRequests: this.pendingRequests.size,
    });

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

      logger.debug('Sending message to worker', {
        type: message.type,
        id,
      });

      this.worker!.postMessage(fullMessage);
    });
  }

  /**
   * Handle incoming messages from worker
   */
  private handleWorkerMessage(event: MessageEvent<WebMCPResponse>): void {
    const response = event.data;

    if (!response || response.id === undefined || response.id === null) {
      logger.warn('Invalid response from worker', { response });
      return;
    }

    const responseId = String(response.id);
    const pending = this.pendingRequests.get(responseId);
    if (!pending) {
      logger.warn('Response for unknown request', { id: response.id });
      return;
    }

    // Clean up
    clearTimeout(pending.timeout);
    this.pendingRequests.delete(responseId);

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
    logger.error('Worker error', { message: error.message });

    // Reject all pending requests
    for (const [, { reject, timeout }] of this.pendingRequests.entries()) {
      clearTimeout(timeout);
      reject(new Error(`Worker error: ${error.message}`));
    }
    this.pendingRequests.clear();
  }

  /**
   * Test worker responsiveness with retry logic
   */

  // pingWithRetry 제거됨

  /**
   * Test worker responsiveness
   */
  async ping(): Promise<string> {
    if (!this.worker) {
      throw new Error('Worker not available');
    }

    const id = createId();
    const message: WebMCPMessage = { type: 'ping', id };

    const pingPromise = new Promise<string>((resolve, reject) => {
      const timeout = setTimeout(() => {
        this.pendingRequests.delete(id);
        reject(new Error(`Ping timeout after ${this.config.timeout}ms`));
      }, this.config.timeout);

      this.pendingRequests.set(id, {
        resolve: resolve as (value: unknown) => void,
        reject,
        timeout,
      });

      this.worker!.postMessage(message);
    });

    // Apply timeout wrapper for additional safety
    return withTimeout(pingPromise, this.config.timeout);
  }

  /**
   * Load an MCP server module in the worker with retry
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
   * List all available tools from loaded servers with retry
   */
  async listAllTools(): Promise<MCPTool[]> {
    const tools = await this.sendMessage<MCPTool[]>({ type: 'listTools' });
    logger.debug('Listed all tools', { toolCount: tools.length });
    return tools;
  }

  /**
   * List tools from a specific server with retry
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
   * Call a tool on an MCP server with retry
   */
  async callTool(
    serverName: string,
    toolName: string,
    args: unknown,
  ): Promise<unknown> {
    logger.debug('Calling tool', { serverName, toolName });
    const result = await this.sendMessage({
      type: 'callTool',
      serverName,
      toolName,
      args,
    });
    logger.debug('Tool call completed', { serverName, toolName, result });
    return result;
  } /**
   * Get proxy status
   */
  getStatus() {
    return {
      initialized: this.isInitialized,
      pendingRequests: this.pendingRequests.size,
      workerType: this.config.workerInstance ? 'instance' : 'path',
      hasWorker: !!this.worker,
    };
  }
}

/**
 * Factory function for creating WebMCP proxy instances
 */
export function createWebMCPProxy(config: WebMCPProxyConfig): WebMCPProxy {
  return new WebMCPProxy(config);
}
