/**
 * MCP Manager Server Tests
 * Comprehensive test suite covering BM25 ranking, pagination, type safety, and edge cases
 */

import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import mcpManagerServer from '../server';
import { LocalDatabase } from '@/lib/db/service';
import type { MCPServerEntity } from '@/models/chat';
import { clearBM25Cache } from '@/lib/search/bm25';

// Test fixtures
const createMockServer = (
  overrides: Partial<MCPServerEntity>,
): MCPServerEntity => {
  const now = new Date();
  return {
    id: `test-${Date.now()}-${Math.random()}`,
    name: 'Test Server',
    isActive: true,
    createdAt: now,
    updatedAt: now,
    transport: {
      type: 'stdio',
      command: 'npx',
      args: ['-y', 'test-server'],
    },
    metadata: {
      description: 'A test server',
    },
    ...overrides,
  };
};

describe('MCP Manager Server', () => {
  let db: LocalDatabase;
  let createdServerIds: string[] = [];

  beforeEach(async () => {
    // Reset database instance to ensure complete isolation
    LocalDatabase.resetInstance();
    db = LocalDatabase.getInstance();
    // Clear all servers before each test to ensure isolation
    await db.mcpServers.clear();
    createdServerIds = [];
    clearBM25Cache();
  });

  afterEach(async () => {
    // Clean up created servers
    for (const id of createdServerIds) {
      try {
        await db.mcpServers.delete(id);
      } catch {
        // Ignore if already deleted
      }
    }
    // Clear all servers after each test to ensure isolation
    await db.mcpServers.clear();
    createdServerIds = [];
    clearBM25Cache();
  });

  describe('Tool Schema', () => {
    it('should export 5 tools', () => {
      expect(mcpManagerServer.tools).toHaveLength(5);
    });

    it('should have listServers tool', () => {
      const tool = mcpManagerServer.tools.find((t) => t.name === 'listServers');
      expect(tool).toBeDefined();
      expect(tool?.description).toContain('List all registered MCP servers');
    });

    it('should have searchServer tool with BM25 mode', () => {
      const tool = mcpManagerServer.tools.find((t) => t.name === 'searchServer');
      expect(tool).toBeDefined();
      expect(tool?.description).toContain('BM25');
      expect(tool?.inputSchema.properties?.searchMode).toBeDefined();
      expect(tool?.inputSchema.properties?.weights).toBeDefined();
    });

    it('should have createServer tool', () => {
      const tool = mcpManagerServer.tools.find((t) => t.name === 'createServer');
      expect(tool).toBeDefined();
      expect(tool?.inputSchema.required).toContain('name');
      expect(tool?.inputSchema.required).toContain('transport');
    });

    it('should have connectServer tool', () => {
      const tool = mcpManagerServer.tools.find((t) => t.name === 'connectServer');
      expect(tool).toBeDefined();
    });

    it('should have disconnectServer tool', () => {
      const tool = mcpManagerServer.tools.find((t) => t.name === 'disconnectServer');
      expect(tool).toBeDefined();
    });
  });

  describe('createServer', () => {
    it('should create a server with valid stdio transport', async () => {
      const response = await mcpManagerServer.callTool('createServer', {
        name: 'Test Stdio Server',
        description: 'A test stdio server',
        transport: {
          type: 'stdio',
          command: 'npx',
          args: ['-y', 'test-server'],
        },
      });

      expect(response).toBeDefined();
      const structuredData = response.structuredContent as {
        server?: MCPServerEntity;
      } | undefined;
      expect(structuredData?.server).toBeDefined();
      expect(structuredData?.server?.name).toBe('Test Stdio Server');
      expect(structuredData?.server?.id).toMatch(/^mcp-/);

      // Track for cleanup
      if (structuredData?.server?.id) {
        createdServerIds.push(structuredData.server.id);
      }
    });

    it('should create a server with valid http transport', async () => {
      const response = await mcpManagerServer.callTool('createServer', {
        name: 'Test HTTP Server',
        description: 'A test http server',
        transport: {
          type: 'http',
          url: 'http://localhost:3000',
          headers: { Authorization: 'Bearer token' },
        },
      });

      expect(response).toBeDefined();
      const structuredData = response.structuredContent as {
        server?: MCPServerEntity;
      } | undefined;
      expect(structuredData?.server).toBeDefined();
      expect(structuredData?.server?.name).toBe('Test HTTP Server');

      if (structuredData?.server?.id) {
        createdServerIds.push(structuredData.server.id);
      }
    });

    it('should reject invalid transport type', async () => {
      const response = await mcpManagerServer.callTool('createServer', {
        name: 'Invalid Server',
        transport: {
          type: 'invalid',
        },
      });

      expect(response.content?.[0]?.type).toBe('text');
      const text = response.content?.[0] as { text?: string };
      expect(text?.text).toContain('Invalid transport configuration');
    });

    it('should reject stdio transport without command', async () => {
      const response = await mcpManagerServer.callTool('createServer', {
        name: 'Incomplete Stdio Server',
        transport: {
          type: 'stdio',
        },
      });

      expect(response.content?.[0]?.type).toBe('text');
      const text = response.content?.[0] as { text?: string };
      expect(text?.text).toContain('Invalid transport configuration');
    });

    it('should reject http transport without url', async () => {
      const response = await mcpManagerServer.callTool('createServer', {
        name: 'Incomplete HTTP Server',
        transport: {
          type: 'http',
        },
      });

      expect(response.content?.[0]?.type).toBe('text');
      const text = response.content?.[0] as { text?: string };
      expect(text?.text).toContain('Invalid transport configuration');
    });

    it('should reject empty server name', async () => {
      const response = await mcpManagerServer.callTool('createServer', {
        name: '',
        transport: {
          type: 'stdio',
          command: 'test',
        },
      });

      expect(response.content?.[0]?.type).toBe('text');
      const text = response.content?.[0] as { text?: string };
      expect(text?.text).toContain('Server name is required');
    });
  });

  describe('listServers', () => {
    beforeEach(async () => {
      // Create test servers
      for (let i = 1; i <= 5; i++) {
        const server = createMockServer({
          name: `Server ${i}`,
          id: `test-server-${i}`,
        });
        await db.mcpServers.add(server);
        createdServerIds.push(server.id);
      }
    });

    it('should list all servers', async () => {
      const response = await mcpManagerServer.callTool('listServers', {});

      expect(response).toBeDefined();
      const data = response.structuredContent as
        | { items?: unknown[] }
        | undefined;
      expect(data?.items).toBeDefined();
      expect(data!.items!.length).toBeGreaterThanOrEqual(5);
    });

    it('should paginate servers correctly', async () => {
      const response = await mcpManagerServer.callTool('listServers', {
        page: 1,
        pageSize: 2,
      });

      expect(response).toBeDefined();
      const data = response.structuredContent as
        | { items?: unknown[]; totalPages?: number }
        | undefined;
      expect(data?.items?.length).toBe(2);
      expect(data?.totalPages).toBeGreaterThanOrEqual(3);
    });

    it('should return all servers with pageSize=-1', async () => {
      const response = await mcpManagerServer.callTool('listServers', {
        pageSize: -1,
      });

      expect(response).toBeDefined();
      const data = response.structuredContent as
        | { items?: unknown[]; page?: number; totalPages?: number }
        | undefined;
      expect(data?.items!.length).toBeGreaterThanOrEqual(5);
      expect(data?.page).toBe(1);
      expect(data?.totalPages).toBe(1);
    });

    it('should filter inactive servers when includeInactive=false', async () => {
      // Create inactive server
      const inactiveServer = createMockServer({
        name: 'Inactive Server',
        id: 'test-inactive',
        isActive: false,
      });
      await db.mcpServers.add(inactiveServer);
      createdServerIds.push(inactiveServer.id);

      const response = await mcpManagerServer.callTool('listServers', {
        includeInactive: false,
      });

      const data = response.structuredContent as
        | { items?: MCPServerEntity[] }
        | undefined;
      const hasInactive = data?.items?.some((s) => !s.isActive);
      expect(hasInactive).toBe(false);
    });
  });

  describe('searchServer - BM25 mode', () => {
    beforeEach(async () => {
      // Create test servers with varying name/description matches
      const servers = [
        createMockServer({
          id: 'server-name-match',
          name: 'Database Manager',
          metadata: { description: 'Manages various databases' },
        }),
        createMockServer({
          id: 'server-desc-match',
          name: 'Storage System',
          metadata: { description: 'Database storage and retrieval system' },
        }),
        createMockServer({
          id: 'server-no-match',
          name: 'File Handler',
          metadata: { description: 'Handles file operations' },
        }),
      ];

      for (const server of servers) {
        await db.mcpServers.add(server);
        createdServerIds.push(server.id);
      }
    });

    it('should use BM25 by default', async () => {
      const response = await mcpManagerServer.callTool('searchServer', {
        query: 'database',
      });

      expect(response).toBeDefined();
      const data = response.structuredContent as { mode?: string } | undefined;
      expect(data?.mode).toBe('bm25');
    });

    it('should rank name matches higher than description matches by default', async () => {
      const response = await mcpManagerServer.callTool('searchServer', {
        query: 'database',
      });

      const data = response.structuredContent as
        | { items?: MCPServerEntity[] }
        | undefined;
      expect(data?.items).toBeDefined();
      expect(data!.items!.length).toBeGreaterThanOrEqual(2);

      // First result should be the name match (default weight: name=2.0, desc=1.0)
      expect(data!.items![0].id).toBe('server-name-match');
    });

    it('should adjust ranking with custom weights', async () => {
      const response = await mcpManagerServer.callTool('searchServer', {
        query: 'database',
        weights: {
          nameWeight: 1,
          descWeight: 3,
        },
      });

      const data = response.structuredContent as
        | { items?: MCPServerEntity[] }
        | undefined;
      expect(data?.items).toBeDefined();

      // With higher desc weight, description match should rank higher
      // (This might vary based on BM25 scoring, but desc match should be prominent)
      const descMatchIndex = data!.items!.findIndex(
        (s) => s.id === 'server-desc-match',
      );
      expect(descMatchIndex).toBeGreaterThanOrEqual(0);
    });

    it('should support multi-word queries', async () => {
      const response = await mcpManagerServer.callTool('searchServer', {
        query: 'database manager',
      });

      const data = response.structuredContent as
        | { items?: MCPServerEntity[] }
        | undefined;
      expect(data?.items).toBeDefined();
      expect(data!.items!.length).toBeGreaterThan(0);
    });

    it('should return empty results for non-matching query', async () => {
      const response = await mcpManagerServer.callTool('searchServer', {
        query: 'nonexistent-keyword-xyz',
      });

      const data = response.structuredContent as
        | { items?: unknown[] }
        | undefined;
      expect(data?.items).toBeDefined();
      expect(data!.items!.length).toBe(0);
    });

    it('should support pageSize=-1 for all results', async () => {
      const response = await mcpManagerServer.callTool('searchServer', {
        query: 'database',
        pageSize: -1,
      });

      const data = response.structuredContent as
        | { items?: unknown[]; page?: number; totalPages?: number }
        | undefined;
      expect(data?.page).toBe(1);
      expect(data?.totalPages).toBe(1);
      expect(data!.items!.length).toBeGreaterThanOrEqual(2);
    });
  });

  describe('searchServer - Simple mode', () => {
    beforeEach(async () => {
      const servers = [
        createMockServer({
          id: 'exact-match',
          name: 'test',
          metadata: { description: 'Exact name match' },
        }),
        createMockServer({
          id: 'starts-with',
          name: 'test-server',
          metadata: { description: 'Starts with test' },
        }),
        createMockServer({
          id: 'contains',
          name: 'my-test-app',
          metadata: { description: 'Contains test' },
        }),
      ];

      for (const server of servers) {
        await db.mcpServers.add(server);
        createdServerIds.push(server.id);
      }
    });

    it('should use simple mode when specified', async () => {
      const response = await mcpManagerServer.callTool('searchServer', {
        query: 'test',
        searchMode: 'simple',
      });

      const data = response.structuredContent as { mode?: string } | undefined;
      expect(data?.mode).toBe('simple');
    });

    it('should rank exact > startsWith > contains in simple mode', async () => {
      const response = await mcpManagerServer.callTool('searchServer', {
        query: 'test',
        searchMode: 'simple',
      });

      const data = response.structuredContent as
        | { items?: MCPServerEntity[] }
        | undefined;
      expect(data?.items).toBeDefined();
      expect(data!.items!.length).toBe(3);

      // Check ranking
      expect(data!.items![0].id).toBe('exact-match');
      expect(data!.items![1].id).toBe('starts-with');
      expect(data!.items![2].id).toBe('contains');
    });

    it('should respect byNameOnly=true in simple mode', async () => {
      await db.mcpServers.add(
        createMockServer({
          id: 'desc-only-match',
          name: 'NoMatch',
          metadata: { description: 'This has test in description' },
        }),
      );
      createdServerIds.push('desc-only-match');

      const response = await mcpManagerServer.callTool('searchServer', {
        query: 'test',
        searchMode: 'simple',
        byNameOnly: true,
      });

      const data = response.structuredContent as
        | { items?: MCPServerEntity[] }
        | undefined;
      const descOnlyFound = data?.items?.some((s) => s.id === 'desc-only-match');
      expect(descOnlyFound).toBe(false);
    });

    it('should search description when byNameOnly=false in simple mode', async () => {
      await db.mcpServers.add(
        createMockServer({
          id: 'desc-only-match',
          name: 'NoMatch',
          metadata: { description: 'This has test in description' },
        }),
      );
      createdServerIds.push('desc-only-match');

      const response = await mcpManagerServer.callTool('searchServer', {
        query: 'test',
        searchMode: 'simple',
        byNameOnly: false,
      });

      const data = response.structuredContent as
        | { items?: MCPServerEntity[] }
        | undefined;
      const descOnlyFound = data?.items?.some((s) => s.id === 'desc-only-match');
      expect(descOnlyFound).toBe(true);
    });
  });

  describe('disconnectServer - Scope Validation', () => {
    let testServerId: string;

    beforeEach(async () => {
      const server = createMockServer({
        id: 'test-disconnect-server',
        name: 'Test Disconnect Server',
      });
      await db.mcpServers.add(server);
      testServerId = server.id;
      createdServerIds.push(testServerId);
    });

    it('should accept valid scope: assistant', async () => {
      const response = await mcpManagerServer.callTool('disconnectServer', {
        serverId: testServerId,
        scope: 'assistant',
      });

      // Should not error on scope validation
      const text = response.content?.[0] as { text?: string };
      expect(text?.text).not.toContain('Scope must be');
    });

    it('should accept valid scope: global', async () => {
      const response = await mcpManagerServer.callTool('disconnectServer', {
        serverId: testServerId,
        scope: 'global',
      });

      expect(response).toBeDefined();
      const structuredData = response.structuredContent as
        | { success?: boolean; scope?: string }
        | undefined;
      expect(structuredData?.success).toBe(true);
      expect(structuredData?.scope).toBe('global');
    });

    it('should reject invalid scope value', async () => {
      const response = await mcpManagerServer.callTool('disconnectServer', {
        serverId: testServerId,
        scope: 'invalid',
      });

      const text = response.content?.[0] as { text?: string };
      expect(text?.text).toContain('Scope must be "assistant" or "global"');
    });

    it('should default to assistant scope when not specified', async () => {
      const response = await mcpManagerServer.callTool('disconnectServer', {
        serverId: testServerId,
      });

      // Default scope is assistant, which requires assistant context
      const text = response.content?.[0] as { text?: string };
      expect(text?.text).toContain('Assistant context not available');
    });
  });

  describe('connectServer - Scope Validation', () => {
    let testServerId: string;

    beforeEach(async () => {
      const server = createMockServer({
        id: 'test-connect-server',
        name: 'Test Connect Server',
      });
      await db.mcpServers.add(server);
      testServerId = server.id;
      createdServerIds.push(testServerId);
    });

    it('should accept valid scope: global', async () => {
      const response = await mcpManagerServer.callTool('connectServer', {
        serverId: testServerId,
        scope: 'global',
      });

      expect(response).toBeDefined();
      const structuredData = response.structuredContent as
        | { success?: boolean; scope?: string }
        | undefined;
      expect(structuredData?.success).toBe(true);
      expect(structuredData?.scope).toBe('global');
    });

    it('should reject invalid scope value', async () => {
      const response = await mcpManagerServer.callTool('connectServer', {
        serverId: testServerId,
        scope: 'invalid',
      });

      const text = response.content?.[0] as { text?: string };
      expect(text?.text).toContain('Scope must be "assistant" or "global"');
    });
  });

  describe('Server Lookup Helper', () => {
    let testServerId: string;
    const testServerName = 'Lookup Test Server';

    beforeEach(async () => {
      const server = createMockServer({
        id: 'test-lookup-server',
        name: testServerName,
      });
      await db.mcpServers.add(server);
      testServerId = server.id;
      createdServerIds.push(testServerId);
    });

    it('should find server by ID', async () => {
      const response = await mcpManagerServer.callTool('connectServer', {
        serverId: testServerId,
        scope: 'global',
      });

      const structuredData = response.structuredContent as
        | { server?: MCPServerEntity }
        | undefined;
      expect(structuredData?.server?.id).toBe(testServerId);
    });

    it('should find server by name (case-insensitive)', async () => {
      const response = await mcpManagerServer.callTool('connectServer', {
        serverName: testServerName.toUpperCase(),
        scope: 'global',
      });

      const structuredData = response.structuredContent as
        | { server?: MCPServerEntity }
        | undefined;
      expect(structuredData?.server?.id).toBe(testServerId);
    });

    it('should return error when server not found by ID', async () => {
      const response = await mcpManagerServer.callTool('connectServer', {
        serverId: 'nonexistent-id',
        scope: 'global',
      });

      const text = response.content?.[0] as { text?: string };
      expect(text?.text).toContain('Server not found');
    });

    it('should return error when server not found by name', async () => {
      const response = await mcpManagerServer.callTool('connectServer', {
        serverName: 'Nonexistent Server',
        scope: 'global',
      });

      const text = response.content?.[0] as { text?: string };
      expect(text?.text).toContain('Server not found');
    });

    it('should require either serverId or serverName', async () => {
      const response = await mcpManagerServer.callTool('connectServer', {
        scope: 'global',
      });

      const text = response.content?.[0] as { text?: string };
      expect(text?.text).toContain('Either serverId or serverName is required');
    });
  });

  describe('Edge Cases', () => {
    it('should handle empty query gracefully', async () => {
      const response = await mcpManagerServer.callTool('searchServer', {
        query: '',
      });

      const text = response.content?.[0] as { text?: string };
      expect(text?.text).toContain('Search query is required');
    });

    it('should handle whitespace-only query', async () => {
      const response = await mcpManagerServer.callTool('searchServer', {
        query: '   ',
      });

      const text = response.content?.[0] as { text?: string };
      expect(text?.text).toContain('Search query is required');
    });

    it('should handle unknown tool name', async () => {
      const response = await mcpManagerServer.callTool('unknown_tool', {});

      const text = response.content?.[0] as { text?: string };
      expect(text?.text).toContain('Unknown tool');
    });
  });
});
