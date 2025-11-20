import { MCPServerEntity } from '@/models/chat';
import { dbService, dbUtils, LocalDatabase } from '@/lib/db/service';
import { getLogger } from '@/lib/logger';
import { Page } from '@/lib/db/types';

const logger = getLogger('McpServerService');

export interface IMcpServerService {
  getAll(): Promise<MCPServerEntity[]>;
  getPage(page: number, pageSize: number): Promise<Page<MCPServerEntity>>;
  getById(id: string): Promise<MCPServerEntity | undefined>;
  getByName(name: string): Promise<MCPServerEntity | undefined>;
  save(server: MCPServerEntity): Promise<MCPServerEntity>;
  saveAll(servers: MCPServerEntity[]): Promise<MCPServerEntity[]>;
  delete(id: string): Promise<void>;
  count(): Promise<number>;
}

export class LocalMcpServerService implements IMcpServerService {
  async getAll(): Promise<MCPServerEntity[]> {
    return dbUtils.getAllMCPServers();
  }

  async getPage(
    page: number,
    pageSize: number,
  ): Promise<Page<MCPServerEntity>> {
    return dbService.mcpServers.getPage(page, pageSize);
  }

  async getById(id: string): Promise<MCPServerEntity | undefined> {
    return dbService.mcpServers.read(id);
  }

  async getByName(name: string): Promise<MCPServerEntity | undefined> {
    return LocalDatabase.getInstance()
      .mcpServers.where('name')
      .equalsIgnoreCase(name)
      .first();
  }

  async save(server: MCPServerEntity): Promise<MCPServerEntity> {
    await dbService.mcpServers.upsert(server);
    return server;
  }

  async saveAll(servers: MCPServerEntity[]): Promise<MCPServerEntity[]> {
    if (servers.length === 0) return [];
    await dbService.mcpServers.upsertMany(servers);
    return servers;
  }

  async delete(id: string): Promise<void> {
    await dbService.mcpServers.delete(id);
  }

  async count(): Promise<number> {
    return dbService.mcpServers.count();
  }
}

export class RemoteMcpServerService implements IMcpServerService {
  constructor(private baseUrl: string) {}

  async getAll(): Promise<MCPServerEntity[]> {
    const response = await fetch(`${this.baseUrl}/mcp-servers`);
    if (!response.ok) throw new Error('Failed to fetch MCP servers');
    return response.json();
  }

  async getPage(
    page: number,
    pageSize: number,
  ): Promise<Page<MCPServerEntity>> {
    const response = await fetch(
      `${this.baseUrl}/mcp-servers?page=${page}&pageSize=${pageSize}`,
    );
    if (!response.ok) throw new Error('Failed to fetch MCP servers page');
    return response.json();
  }

  async getById(id: string): Promise<MCPServerEntity | undefined> {
    const response = await fetch(`${this.baseUrl}/mcp-servers/${id}`);
    if (response.status === 404) return undefined;
    if (!response.ok) throw new Error('Failed to fetch MCP server');
    return response.json();
  }

  async getByName(name: string): Promise<MCPServerEntity | undefined> {
    const response = await fetch(
      `${this.baseUrl}/mcp-servers/by-name/${encodeURIComponent(name)}`,
    );
    if (response.status === 404) return undefined;
    if (!response.ok) throw new Error('Failed to fetch MCP server by name');
    return response.json();
  }

  async save(server: MCPServerEntity): Promise<MCPServerEntity> {
    const response = await fetch(`${this.baseUrl}/mcp-servers`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(server),
    });
    if (!response.ok) throw new Error('Failed to save MCP server');
    return response.json();
  }

  async saveAll(servers: MCPServerEntity[]): Promise<MCPServerEntity[]> {
    // Remote API doesn't support bulk save yet, so we do sequential
    const saved: MCPServerEntity[] = [];
    for (const server of servers) {
      saved.push(await this.save(server));
    }
    return saved;
  }

  async delete(id: string): Promise<void> {
    const response = await fetch(`${this.baseUrl}/mcp-servers/${id}`, {
      method: 'DELETE',
    });
    if (!response.ok) throw new Error('Failed to delete MCP server');
  }

  async count(): Promise<number> {
    const response = await fetch(`${this.baseUrl}/mcp-servers/count`);
    if (!response.ok) throw new Error('Failed to count MCP servers');
    const data = await response.json();
    return data.count;
  }
}

export class McpServerService implements IMcpServerService {
  private localService: LocalMcpServerService;
  private remoteService: RemoteMcpServerService | null = null;

  constructor(agentHubUrl?: string) {
    this.localService = new LocalMcpServerService();
    if (agentHubUrl) {
      this.remoteService = new RemoteMcpServerService(agentHubUrl);
    }
  }

  async getAll(): Promise<MCPServerEntity[]> {
    if (this.remoteService) {
      try {
        const remoteServers = await this.remoteService.getAll();
        // Sync to local (batch operation)
        await this.localService.saveAll(remoteServers);
        return remoteServers;
      } catch (error) {
        logger.error(
          'Failed to fetch from remote, falling back to local',
          error,
        );
        return this.localService.getAll();
      }
    }
    return this.localService.getAll();
  }

  async getPage(
    page: number,
    pageSize: number,
  ): Promise<Page<MCPServerEntity>> {
    if (this.remoteService) {
      try {
        return await this.remoteService.getPage(page, pageSize);
      } catch (error) {
        logger.error(
          'Failed to fetch page from remote, falling back to local',
          error,
        );
        return this.localService.getPage(page, pageSize);
      }
    }
    return this.localService.getPage(page, pageSize);
  }

  async getById(id: string): Promise<MCPServerEntity | undefined> {
    if (this.remoteService) {
      try {
        return await this.remoteService.getById(id);
      } catch (error) {
        logger.error(
          'Failed to fetch from remote, falling back to local',
          error,
        );
        return this.localService.getById(id);
      }
    }
    return this.localService.getById(id);
  }

  async getByName(name: string): Promise<MCPServerEntity | undefined> {
    if (this.remoteService) {
      try {
        return await this.remoteService.getByName(name);
      } catch (error) {
        logger.error(
          'Failed to fetch from remote, falling back to local',
          error,
        );
        return this.localService.getByName(name);
      }
    }
    return this.localService.getByName(name);
  }

  async save(server: MCPServerEntity): Promise<MCPServerEntity> {
    if (this.remoteService) {
      try {
        const saved = await this.remoteService.save(server);
        await this.localService.save(saved);
        return saved;
      } catch (error) {
        logger.error('Failed to save to remote', error);
        throw error;
      }
    }
    return this.localService.save(server);
  }

  async saveAll(servers: MCPServerEntity[]): Promise<MCPServerEntity[]> {
    if (this.remoteService) {
      try {
        const saved = await this.remoteService.saveAll(servers);
        await this.localService.saveAll(saved);
        return saved;
      } catch (error) {
        logger.error('Failed to save to remote', error);
        throw error;
      }
    }
    return this.localService.saveAll(servers);
  }

  async delete(id: string): Promise<void> {
    if (this.remoteService) {
      try {
        await this.remoteService.delete(id);
      } catch (error) {
        logger.error('Failed to delete from remote', error);
        throw error;
      }

      try {
        await this.localService.delete(id);
      } catch (error) {
        logger.error(
          'Remote deletion succeeded, but failed to delete from local',
          error,
        );
        // Do not throw; treat as success, but log for reconciliation
      }
    } else {
      await this.localService.delete(id);
    }
  }

  async count(): Promise<number> {
    if (this.remoteService) {
      try {
        return await this.remoteService.count();
      } catch (error) {
        logger.error(
          'Failed to count from remote, falling back to local',
          error,
        );
        return this.localService.count();
      }
    }
    return this.localService.count();
  }
}
