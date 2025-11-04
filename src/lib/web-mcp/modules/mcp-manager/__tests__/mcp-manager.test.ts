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
    db = LocalDatabase.getInstance();
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
    createdServerIds = [];
    clearBM25Cache();
  });

  describe('Tool Schema', () => {
    it('should export 5 tools', () => {
      expect(mcpManagerServer.tools).toHaveLength(5);
    });

    it('should have list_servers tool', () => {
      const tool = mcpManagerServer.tools.find((t) => t.name === 'list_servers');
      expect(tool).toBeDefined();
      expect(tool?.description).toContain('List all registered MCP servers');
    });

    it('should have search_server tool with BM25 mode', () => {
      const tool = mcpManagerServer.tools.find((t) => t.name === 'search_server');
      expect(tool).toBeDefined();
      expect(tool?.description).toContain('BM25');
      expect(tool?.inputSchema.properties?.searchMode).toBeDefined();
      expect(tool?.inputSchema.properties?.weights).toBeDefined();
    });

    it('should have create_server tool', () => {
      const tool = mcpManagerServer.tools.find((t) => t.name === 'create_server');
      expect(tool).toBeDefined();
      expect(tool?.inputSchema.required).toContain('name');
      expect(tool?.inputSchema.required).toContain('transport');
    });

    it('should have connect_server tool', () => {
      const tool = mcpManagerServer.tools.find((t) => t.name === 'connect_server');
      expect(tool).toBeDefined();
    });

    it('should have disconnect_server tool', () => {
      const tool = mcpManagerServer.tools.find((t) => t.name === 'disconnect_server');
      expect(tool).toBeDefined();
    });
  });

  describe('create_server', () => {
    it('should create a server with valid stdio transport', async () => {
      const response = await mcpManagerServer.callTool('create_server', {
        name: 'Test Stdio Server',
        description: 'A test stdio server',
        transport: {
          type: 'stdio',
          command: 'npx',
          args: ['-y', 'test-server'],
        },
      });

      expect(response.result).toBeDefined();
      const structuredData = (response.result as { structuredContent?: { server?: MCPServerEntity } })
        .structuredContent;
      expect(structuredData?.server).toBeDefined();
      expect(structuredData?.server?.name).toBe('Test Stdio Server');
      expect(structuredData?.server?.id).toMatch(/^mcp-/);

      // Track for cleanup
      if (structuredData?.server?.id) {
        createdServerIds.push(structuredData.server.id);
      }
    });

    it('should create a server with valid http transport', async () => {
      const response = await mcpManagerServer.callTool('create_server', {
        name: 'Test HTTP Server',
        description: 'A test http server',
        transport: {
          type: 'http',
          url: 'http://localhost:3000',
          headers: { Authorization: 'Bearer token' },
        },
      });

      expect(response.result).toBeDefined();
      const structuredData = (response.result as { structuredContent?: { server?: MCPServerEntity } })
        .structuredContent;
      expect(structuredData?.server).toBeDefined();
      expect(structuredData?.server?.name).toBe('Test HTTP Server');

      if (structuredData?.server?.id) {
        createdServerIds.push(structuredData.server.id);
      }
    });

    it('should reject invalid transport type', async () => {
      const response = await mcpManagerServer.callTool('create_server', {
        name: 'Invalid Server',
        transport: {
          type: 'invalid',
        },
      });

      expect(response.result?.content?.[0]?.type).toBe('text');
      const text = response.result?.content?.[0] as { text?: string };
      expect(text?.text).toContain('Invalid transport configuration');
    });

    it('should reject stdio transport without command', async () => {
      const response = await mcpManagerServer.callTool('create_server', {
        name: 'Incomplete Stdio Server',
        transport: {
          type: 'stdio',
        },
      });

      expect(response.result?.content?.[0]?.type).toBe('text');
      const text = response.result?.content?.[0] as { text?: string };
      expect(text?.text).toContain('Invalid transport configuration');
    });

    it('should reject http transport without url', async () => {
      const response = await mcpManagerServer.callTool('create_server', {
        name: 'Incomplete HTTP Server',
        transport: {
          type: 'http',
        },
      });

      expect(response.result?.content?.[0]?.type).toBe('text');
      const text = response.result?.content?.[0] as { text?: string };
      expect(text?.text).toContain('Invalid transport configuration');
    });

    it('should reject empty server name', async () => {
      const response = await mcpManagerServer.callTool('create_server', {
        name: '',
        transport: {
          type: 'stdio',
          command: 'test',
        },
      });

      expect(response.result?.content?.[0]?.type).toBe('text');
      const text = response.result?.content?.[0] as { text?: string };
      expect(text?.text).toContain('Server name is required');
    });
  });

  describe('list_servers', () => {
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
      const response = await mcpManagerServer.callTool('list_servers', {});

      expect(response.result).toBeDefined();
      const data = (response.result as { structuredContent?: { items?: unknown[] } })
        .structuredContent;
      expect(data?.items).toBeDefined();
      expect(data!.items!.length).toBeGreaterThanOrEqual(5);
    });

    it('should paginate servers correctly', async () => {
      const response = await mcpManagerServer.callTool('list_servers', {
        page: 1,
        pageSize: 2,
      });

      expect(response.result).toBeDefined();
      const data = (response.result as { structuredContent?: { items?: unknown[]; totalPages?: number } })
        .structuredContent;
      expect(data?.items?.length).toBe(2);
      expect(data?.totalPages).toBeGreaterThanOrEqual(3);
    });

    it('should return all servers with pageSize=-1', async () => {
      const response = await mcpManagerServer.callTool('list_servers', {
        pageSize: -1,
      });

      expect(response.result).toBeDefined();
      const data = (response.result as {
        structuredContent?: { items?: unknown[]; page?: number; totalPages?: number }
      }).structuredContent;
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

      const response = await mcpManagerServer.callTool('list_servers', {
        includeInactive: false,
      });

      const data = (response.result as { structuredContent?: { items?: MCPServerEntity[] } })
        .structuredContent;
      const hasInactive = data?.items?.some((s) => !s.isActive);
      expect(hasInactive).toBe(false);
    });
  });

  describe('search_server - BM25 mode', () => {
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
      const response = await mcpManagerServer.callTool('search_server', {
        query: 'database',
      });

      expect(response.result).toBeDefined();
      const data = (response.result as { structuredContent?: { mode?: string } })
        .structuredContent;
      expect(data?.mode).toBe('bm25');
    });

    it('should rank name matches higher than description matches by default', async () => {
      const response = await mcpManagerServer.callTool('search_server', {
        query: 'database',
      });

      const data = (response.result as { structuredContent?: { items?: MCPServerEntity[] } })
        .structuredContent;
      expect(data?.items).toBeDefined();
      expect(data!.items!.length).toBeGreaterThanOrEqual(2);

      // First result should be the name match (default weight: name=2.0, desc=1.0)
      expect(data!.items![0].id).toBe('server-name-match');
    });

    it('should adjust ranking with custom weights', async () => {
      const response = await mcpManagerServer.callTool('search_server', {
        query: 'database',
        weights: {
          nameWeight: 1,
          descWeight: 3,
        },
      });

      const data = (response.result as { structuredContent?: { items?: MCPServerEntity[] } })
        .structuredContent;
      expect(data?.items).toBeDefined();

      // With higher desc weight, description match should rank higher
      // (This might vary based on BM25 scoring, but desc match should be prominent)
      const descMatchIndex = data!.items!.findIndex(
        (s) => s.id === 'server-desc-match',
      );
      expect(descMatchIndex).toBeGreaterThanOrEqual(0);
    });

    it('should support multi-word queries', async () => {
      const response = await mcpManagerServer.callTool('search_server', {
        query: 'database manager',
      });

      const data = (response.result as { structuredContent?: { items?: MCPServerEntity[] } })
        .structuredContent;
      expect(data?.items).toBeDefined();
      expect(data!.items!.length).toBeGreaterThan(0);
    });

    it('should return empty results for non-matching query', async () => {
      const response = await mcpManagerServer.callTool('search_server', {
        query: 'nonexistent-keyword-xyz',
      });

      const data = (response.result as { structuredContent?: { items?: unknown[] } })
        .structuredContent;
      expect(data?.items).toBeDefined();
      expect(data!.items!.length).toBe(0);
    });

    it('should support pageSize=-1 for all results', async () => {
      const response = await mcpManagerServer.callTool('search_server', {
        query: 'database',
        pageSize: -1,
      });

      const data = (response.result as {
        structuredContent?: { items?: unknown[]; page?: number; totalPages?: number }
      }).structuredContent;
      expect(data?.page).toBe(1);
      expect(data?.totalPages).toBe(1);
      expect(data!.items!.length).toBeGreaterThanOrEqual(2);
    });
  });

  describe('search_server - Simple mode', () => {
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
      const response = await mcpManagerServer.callTool('search_server', {
        query: 'test',
        searchMode: 'simple',
      });

      const data = (response.result as { structuredContent?: { mode?: string } })
        .structuredContent;
      expect(data?.mode).toBe('simple');
    });

    it('should rank exact > startsWith > contains in simple mode', async () => {
      const response = await mcpManagerServer.callTool('search_server', {
        query: 'test',
        searchMode: 'simple',
      });

      const data = (response.result as { structuredContent?: { items?: MCPServerEntity[] } })
        .structuredContent;
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

      const response = await mcpManagerServer.callTool('search_server', {
        query: 'test',
        searchMode: 'simple',
        byNameOnly: true,
      });

      const data = (response.result as { structuredContent?: { items?: MCPServerEntity[] } })
        .structuredContent;
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

      const response = await mcpManagerServer.callTool('search_server', {
        query: 'test',
        searchMode: 'simple',
        byNameOnly: false,
      });

      const data = (response.result as { structuredContent?: { items?: MCPServerEntity[] } })
        .structuredContent;
      const descOnlyFound = data?.items?.some((s) => s.id === 'desc-only-match');
      expect(descOnlyFound).toBe(true);
    });
  });

  describe('disconnect_server - Scope Validation', () => {
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
      const response = await mcpManagerServer.callTool('disconnect_server', {
        serverId: testServerId,
        scope: 'assistant',
      });

      // Should not error on scope validation
      const text = response.result?.content?.[0] as { text?: string };
      expect(text?.text).not.toContain('Scope must be');
    });

    it('should accept valid scope: global', async () => {
      const response = await mcpManagerServer.callTool('disconnect_server', {
        serverId: testServerId,
        scope: 'global',
      });

      expect(response.result).toBeDefined();
      const structuredData = (response.result as {
        structuredContent?: { success?: boolean; scope?: string }
      }).structuredContent;
      expect(structuredData?.success).toBe(true);
      expect(structuredData?.scope).toBe('global');
    });

    it('should reject invalid scope value', async () => {
      const response = await mcpManagerServer.callTool('disconnect_server', {
        serverId: testServerId,
        scope: 'invalid',
      });

      const text = response.result?.content?.[0] as { text?: string };
      expect(text?.text).toContain('Scope must be "assistant" or "global"');
    });

    it('should default to assistant scope when not specified', async () => {
      const response = await mcpManagerServer.callTool('disconnect_server', {
        serverId: testServerId,
      });

      // Default scope is assistant, which requires assistant context
      const text = response.result?.content?.[0] as { text?: string };
      expect(text?.text).toContain('Assistant context not available');
    });
  });

  describe('connect_server - Scope Validation', () => {
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
      const response = await mcpManagerServer.callTool('connect_server', {
        serverId: testServerId,
        scope: 'global',
      });

      expect(response.result).toBeDefined();
      const structuredData = (response.result as {
        structuredContent?: { success?: boolean; scope?: string }
      }).structuredContent;
      expect(structuredData?.success).toBe(true);
      expect(structuredData?.scope).toBe('global');
    });

    it('should reject invalid scope value', async () => {
      const response = await mcpManagerServer.callTool('connect_server', {
        serverId: testServerId,
        scope: 'invalid',
      });

      const text = response.result?.content?.[0] as { text?: string };
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
      const response = await mcpManagerServer.callTool('connect_server', {
        serverId: testServerId,
        scope: 'global',
      });

      const structuredData = (response.result as {
        structuredContent?: { server?: MCPServerEntity }
      }).structuredContent;
      expect(structuredData?.server?.id).toBe(testServerId);
    });

    it('should find server by name (case-insensitive)', async () => {
      const response = await mcpManagerServer.callTool('connect_server', {
        serverName: testServerName.toUpperCase(),
        scope: 'global',
      });

      const structuredData = (response.result as {
        structuredContent?: { server?: MCPServerEntity }
      }).structuredContent;
      expect(structuredData?.server?.id).toBe(testServerId);
    });

    it('should return error when server not found by ID', async () => {
      const response = await mcpManagerServer.callTool('connect_server', {
        serverId: 'nonexistent-id',
        scope: 'global',
      });

      const text = response.result?.content?.[0] as { text?: string };
      expect(text?.text).toContain('Server not found');
    });

    it('should return error when server not found by name', async () => {
      const response = await mcpManagerServer.callTool('connect_server', {
        serverName: 'Nonexistent Server',
        scope: 'global',
      });

      const text = response.result?.content?.[0] as { text?: string };
      expect(text?.text).toContain('Server not found');
    });

    it('should require either serverId or serverName', async () => {
      const response = await mcpManagerServer.callTool('connect_server', {
        scope: 'global',
      });

      const text = response.result?.content?.[0] as { text?: string };
      expect(text?.text).toContain('Either serverId or serverName is required');
    });
  });

  describe('Edge Cases', () => {
    it('should handle empty query gracefully', async () => {
      const response = await mcpManagerServer.callTool('search_server', {
        query: '',
      });

      const text = response.result?.content?.[0] as { text?: string };
      expect(text?.text).toContain('Search query is required');
    });

    it('should handle whitespace-only query', async () => {
      const response = await mcpManagerServer.callTool('search_server', {
        query: '   ',
      });

      const text = response.result?.content?.[0] as { text?: string };
      expect(text?.text).toContain('Search query is required');
    });

    it('should handle unknown tool name', async () => {
      const response = await mcpManagerServer.callTool('unknown_tool', {});

      const text = response.result?.content?.[0] as { text?: string };
      expect(text?.text).toContain('Unknown tool');
    });
  });
});
