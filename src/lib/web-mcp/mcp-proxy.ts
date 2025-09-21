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
  WebMCPProxyConfig,
  MCPTool,
  MCPResponse,
} from '../mcp-types';
import { getLogger } from '../logger';

const logger = getLogger('WebMCPProxy');

/**
 * A proxy class for communicating with an MCP server running in a Web Worker.
 * It handles the creation and termination of the worker, sending messages,
 * and managing the lifecycle of requests.
 */
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

  /**
   * Initializes a new instance of the `WebMCPProxy`.
   * @param config The configuration for the proxy, including the worker path or instance.
   */
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

  /**
   * The internal implementation of the initialization logic.
   * @private
   */
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
   * Ensures the proxy is initialized before sending a message.
   * @private
   */
  private async ensureInitialization(): Promise<void> {
    // The public `initialize` method is already idempotent.
    await this.initialize();
  }

  /**
   * Cleans up all resources used by the proxy, including terminating the worker
   * and rejecting any pending requests.
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
   * @template T The expected type of the response.
   * @param message The message to send, without the `id` property.
   * @param isInitPing A flag to bypass initialization check for the initial ping.
   * @returns A promise that resolves with the response from the worker.
   * @private
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

  /**
   * Handles incoming messages from the worker, resolving or rejecting the
   * corresponding pending request.
   * @param event The `MessageEvent` from the worker.
   * @private
   */
  private handleWorkerMessage(event: MessageEvent<MCPResponse<unknown>>): void {
    const response = event.data;

    logger.debug('Received worker message', {
      hasResponse: !!response,
      responseId: response?.id,
      responseKeys: response ? Object.keys(response) : [],
      hasError: response && 'error' in response,
      hasResult: response && 'result' in response,
      errorStructure: response?.error ? Object.keys(response.error) : undefined,
    });

    if (!response || response.id === undefined || response.id === null) {
      logger.warn('Invalid worker response - missing id', { response });
      return;
    }

    const pending = this.pendingRequests.get(String(response.id));
    if (!pending) {
      logger.warn('No pending request found for response', {
        responseId: response.id,
      });
      return;
    }

    clearTimeout(pending.timeout);
    this.pendingRequests.delete(String(response.id));

    try {
      if (response.error) {
        const errorMessage =
          typeof response.error === 'string'
            ? response.error
            : response.error.message || 'Unknown error';
        logger.error('Worker returned error', {
          responseId: response.id,
          error: response.error,
          errorMessage,
        });
        pending.reject(new Error(errorMessage));
      } else {
        logger.debug('Worker returned success', {
          responseId: response.id,
          hasResult: !!response.result,
        });
        pending.resolve(response);
      }
    } catch (handleError) {
      logger.error('Error handling worker message', {
        responseId: response.id,
        handleError,
        response,
      });
      pending.reject(
        handleError instanceof Error
          ? handleError
          : new Error(String(handleError)),
      );
    }
  }

  /**
   * Handles errors from the worker, rejecting all pending requests.
   * @param error The `ErrorEvent` from the worker.
   * @private
   */
  private handleWorkerError(error: ErrorEvent): void {
    logger.error('Worker error', { message: error.message });
    for (const [, { reject, timeout }] of this.pendingRequests.entries()) {
      clearTimeout(timeout);
      reject(new Error(`Worker error: ${error.message}`));
    }
    this.pendingRequests.clear();
  }

  /**
   * A helper method to parse the `structuredContent` from an MCP response.
   * @template T The expected type of the structured content.
   * @param response The raw `MCPResponse` from the worker.
   * @returns The parsed `structuredContent`.
   * @throws An error if the response has no result or structured content.
   * @private
   */
  private parseResponse<T = unknown>(response: MCPResponse<T>): T {
    // Prefer structured data when available

    if (response.result && response.result.structuredContent) {
      return response.result?.structuredContent as T;
    }
    throw new Error('No structured content available in MCP response');
  }

  /**
   * Pings the worker to check if it's alive and responsive.
   * @returns A promise that resolves with the response from the worker (typically 'pong').
   */
  async ping(): Promise<string> {
    const response = await this.sendMessage<MCPResponse<unknown>>({
      type: 'ping',
    });
    const result = this.parseResponse<string>(response);
    return result || 'pong';
  }

  /**
   * Instructs the worker to load a specific MCP server module.
   * @param serverName The name of the server to load.
   * @returns A promise that resolves with information about the loaded server.
   */
  async loadServer(serverName: string): Promise<{
    name: string;
    description?: string;
    version?: string;
    toolCount: number;
  }> {
    const response = await this.sendMessage<MCPResponse<unknown>>({
      type: 'loadServer',
      serverName,
    });
    return this.parseResponse<{
      name: string;
      description?: string;
      version?: string;
      toolCount: number;
    }>(response);
  }

  /**
   * Lists all tools available from all loaded servers in the worker.
   * @returns A promise that resolves to an array of `MCPTool` objects.
   */
  async listAllTools(): Promise<MCPTool[]> {
    const response = await this.sendMessage<MCPResponse<unknown>>({
      type: 'listTools',
    });
    const result = this.parseResponse<MCPTool[]>(response);
    return Array.isArray(result) ? result : [];
  }

  /**
   * Lists the tools available from a specific loaded server in the worker.
   * @param serverName The name of the server to get tools from.
   * @returns A promise that resolves to an array of `MCPTool` objects.
   */
  async listTools(serverName: string): Promise<MCPTool[]> {
    const response = await this.sendMessage<MCPResponse<unknown>>({
      type: 'listTools',
      serverName,
    });
    const result = this.parseResponse<MCPTool[]>(response);
    return Array.isArray(result) ? result : [];
  }

  /**
   * Calls a tool on a specific server within the worker.
   * @param serverName The name of the server.
   * @param toolName The name of the tool to call.
   * @param args The arguments for the tool.
   * @returns A promise that resolves to the raw `MCPResponse` from the tool call.
   */
  async callTool(
    serverName: string,
    toolName: string,
    args: Record<string, unknown> = {},
  ): Promise<MCPResponse<unknown>> {
    return this.sendMessage<MCPResponse<unknown>>({
      type: 'callTool',
      serverName,
      toolName,
      args,
    });
  }

  /**
   * Gets the service context from a specific server within the worker.
   * @param serverName The name of the server.
   * @returns A promise that resolves to the service context string.
   */
  async getServiceContext(serverName: string): Promise<string> {
    const response = await this.sendMessage<MCPResponse<unknown>>({
      type: 'getServiceContext',
      serverName,
    });
    const result = this.parseResponse<string>(response);
    return typeof result === 'string' ? result : '';
  }
}
