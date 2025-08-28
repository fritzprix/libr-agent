import { useCallback, useEffect, useRef, useState } from 'react';
import { getLogger } from '../lib/logger';
import { WebMCPProxy, createWebMCPProxy } from '../lib/web-mcp/mcp-proxy';
import { MCPTool } from '../lib/mcp-types';

// Vite Worker import
import MCPWorker from '../lib/web-mcp/mcp-worker.ts?worker';

const logger = getLogger('WebMCPServer');

// 서버 프록시 인터페이스
export interface WebMCPServerProxy {
  name: string;
  isLoaded: boolean;
  tools: MCPTool[];
  [methodName: string]: unknown;
}

// 전역 프록시 인스턴스 관리
class WebMCPProxyManager {
  private static instance: WebMCPProxyManager | null = null;
  private proxy: WebMCPProxy | null = null;
  private initialized = false;
  private serverProxies = new Map<string, WebMCPServerProxy>();

  static getInstance(): WebMCPProxyManager {
    if (!this.instance) {
      this.instance = new WebMCPProxyManager();
    }
    return this.instance;
  }

  async ensureInitialized(): Promise<WebMCPProxy> {
    if (this.proxy && this.initialized) {
      return this.proxy;
    }

    if (this.proxy) {
      return this.proxy;
    }

    logger.debug('Initializing WebMCP proxy');
    const workerInstance = new MCPWorker();
    this.proxy = createWebMCPProxy({ workerInstance });
    await this.proxy.initialize();
    this.initialized = true;
    logger.info('WebMCP proxy initialized');

    return this.proxy;
  }

  async getServerProxy<T extends WebMCPServerProxy>(
    serverName: string,
  ): Promise<T> {
    const cachedProxy = this.serverProxies.get(serverName);
    if (cachedProxy) {
      return cachedProxy as T;
    }

    const proxy = await this.ensureInitialized();

    // 서버 로드
    await proxy.loadServer(serverName);
    const tools = await proxy.listTools(serverName);

    // 서버 프록시 생성
    const serverProxy: WebMCPServerProxy = {
      name: serverName,
      isLoaded: true,
      tools,
    };

    // 각 툴을 메서드로 동적 생성
    tools.forEach((tool) => {
      const methodName = tool.name.startsWith(`${serverName}__`)
        ? tool.name.replace(`${serverName}__`, '')
        : tool.name;

      serverProxy[methodName] = async (args?: unknown) => {
        let safeArgs: Record<string, unknown> | undefined;
        if (typeof args === 'undefined') {
          safeArgs = undefined;
        } else if (
          typeof args === 'object' &&
          args !== null &&
          !Array.isArray(args)
        ) {
          safeArgs = args as Record<string, unknown>;
        } else if (typeof args === 'string') {
          try {
            safeArgs = args.length
              ? (JSON.parse(args) as Record<string, unknown>)
              : {};
          } catch {
            safeArgs = { raw: args };
          }
        } else {
          safeArgs = { value: args };
        }
        const result = await proxy.callTool(serverName, methodName, safeArgs);
        return result;
      };
    });

    this.serverProxies.set(serverName, serverProxy);
    logger.info('Created server proxy', {
      serverName,
      toolCount: tools.length,
    });

    return serverProxy as T;
  }

  cleanup() {
    if (this.proxy) {
      this.proxy.cleanup();
      this.proxy = null;
    }
    this.initialized = false;
    this.serverProxies.clear();
  }
}

// 간소화된 useWebMCPServer hook
export function useWebMCPServer<T extends WebMCPServerProxy>(
  serverName: string,
) {
  const [server, setServer] = useState<T | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const proxyManagerRef = useRef(WebMCPProxyManager.getInstance());

  const loadServerProxy = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const serverProxy =
        await proxyManagerRef.current.getServerProxy<T>(serverName);
      setServer(serverProxy);
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      setError(errorMessage);
      logger.error('Failed to load server proxy', { serverName, error: err });
    } finally {
      setLoading(false);
    }
  }, [serverName]);

  // 자동 로드
  useEffect(() => {
    if (!server) {
      loadServerProxy();
    }
  }, [server, loadServerProxy]);

  // 정리
  useEffect(() => {
    return () => {
      // 컴포넌트 언마운트시 정리는 하지 않음 (전역 인스턴스 유지)
    };
  }, []);

  return {
    server,
    loading,
    error,
    reload: loadServerProxy,
  } as const;
}
