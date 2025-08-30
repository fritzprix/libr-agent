/**
 * 🌐 Web Worker MCP Proxy
 *
 * Provides a clean, robust interface for communicating with MCP servers
 * running in web workers. It features lazy initialization, ensuring that the
 * worker is started automatically on the first method call.
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
  private config: WebMCPProxyConfig & {
    timeout: number;
  };
  private isInitialized = false;
  private initializationPromise: Promise<void> | null = null;

  constructor(config: WebMCPProxyConfig) {
    this.config = {
      timeout: 30000, // 30 seconds default
      ...config,
    };
  }

  /**
   * Explicitly initializes the proxy. This method is idempotent and safe to call multiple times.
   * If not called manually, it will be invoked automatically by the first API call.
   */
  async initialize(): Promise<void> {
    if (this.isInitialized) {
      return;
    }
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
      logger.info('Initializing WebMCP proxy...');
      if (this.config.workerInstance) {
        this.worker = this.config.workerInstance;
      } else if (this.config.workerPath) {
        this.worker = new Worker(this.config.workerPath, { type: 'module' });
      } else {
        throw new Error('Either workerInstance or workerPath must be provided');
      }

      this.worker.onmessage = this.handleWorkerMessage.bind(this);
      this.worker.onerror = this.handleWorkerError.bind(this);

      // Ping the worker to confirm it's responsive.
      await this.sendMessage<string>({ type: 'ping' }, true);

      this.isInitialized = true;
      logger.info('WebMCP proxy initialized successfully');
    } catch (error) {
      const errorMessage =
        error instanceof Error ? error.message : String(error);
      logger.error('Failed to initialize WebMCP proxy', {
        error: errorMessage,
      });
      this.cleanup(); // Cleanup on initialization failure
      throw new Error(`Failed to initialize WebMCP proxy: ${errorMessage}`);
    }
  }

  /**
   * Ensures the proxy is initialized. If not, it starts the initialization
   * and waits for it to complete.
   */
  private async ensureInitialization(): Promise<void> {
    // The public `initialize` method is already idempotent.
    await this.initialize();
  }

  /**
   * Cleans up resources, terminates the worker, and rejects all pending requests.
   */
  cleanup(): void {
    logger.debug('Cleaning up WebMCP proxy', {
      pendingRequests: this.pendingRequests.size,
    });
    for (const [, { reject, timeout }] of this.pendingRequests.entries()) {
      clearTimeout(timeout);
      reject(new Error('Worker terminated'));
    }
    this.pendingRequests.clear();

    if (this.worker) {
      this.worker.terminate();
      this.worker = null;
    }
    this.isInitialized = false;
  }

  /**
   * The core communication method. Sends a message to the worker and awaits a response.
   */
  private async sendMessage<T = unknown>(
    message: Omit<WebMCPMessage, 'id'>,
    isInitPing = false,
  ): Promise<T> {
    // For the special ping inside `_doInitialize`, skip the full initialization check.
    if (!isInitPing) {
      await this.ensureInitialization();
    }

    // At this point, the worker object must exist.
    const worker = this.worker!;
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

      worker.postMessage(fullMessage);
    });
  }

  private handleWorkerMessage(event: MessageEvent<WebMCPResponse>): void {
    const response = event.data;
    if (!response || response.id === undefined || response.id === null) {
      return;
    }

    const pending = this.pendingRequests.get(String(response.id));
    if (!pending) {
      return;
    }

    clearTimeout(pending.timeout);
    this.pendingRequests.delete(String(response.id));

    if (response.error) {
      pending.reject(new Error(response.error));
    } else {
      pending.resolve(response.result);
    }
  }

  private handleWorkerError(error: ErrorEvent): void {
    logger.error('Worker error', { message: error.message });
    for (const [, { reject, timeout }] of this.pendingRequests.entries()) {
      clearTimeout(timeout);
      reject(new Error(`Worker error: ${error.message}`));
    }
    this.pendingRequests.clear();
  }

  async ping(): Promise<string> {
    return this.sendMessage<string>({ type: 'ping' });
  }

  async loadServer(
    serverName: string,
  ): Promise<{
    name: string;
    description?: string;
    version?: string;
    toolCount: number;
  }> {
    return this.sendMessage({ type: 'loadServer', serverName });
  }

  async listAllTools(): Promise<MCPTool[]> {
    return this.sendMessage<MCPTool[]>({ type: 'listTools' });
  }

  async listTools(serverName: string): Promise<MCPTool[]> {
    return this.sendMessage<MCPTool[]>({ type: 'listTools', serverName });
  }

  async callTool<T = unknown>(
    serverName: string,
    toolName: string,
    args: Record<string, unknown> = {},
  ): Promise<T> {
    return this.sendMessage<T>({
      type: 'callTool',
      serverName,
      toolName,
      args,
    });
  }

  async getServiceContext(serverName: string): Promise<string> {
    return this.sendMessage<string>({ type: 'getServiceContext', serverName });
  }
}
