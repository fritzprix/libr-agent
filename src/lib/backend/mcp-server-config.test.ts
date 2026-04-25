import { beforeEach, describe, expect, it, vi } from 'vitest';
import { safeInvoke } from './core';
import { upsertMCPServer } from './mcp-server-config';
import type { MCPServerEntity } from '@/models/chat';

vi.mock('./core', () => ({
  safeInvoke: vi.fn(),
}));

function createServer(overrides: Partial<MCPServerEntity> = {}): MCPServerEntity {
  const now = new Date('2026-04-19T00:00:00.000Z');

  return {
    id: 'temp-client-id',
    name: 'huggingface',
    isActive: true,
    createdAt: now,
    updatedAt: now,
    transport: {
      type: 'stdio',
      command: 'npx',
      args: ['-y', '@fre4x/huggingface'],
      env: {
        HF_ALLOW_REMOTE_MODELS: 'true',
      },
    },
    ...overrides,
  };
}

describe('mcp-server-config upsert', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('creates a server when no server with the same name exists', async () => {
    vi.mocked(safeInvoke)
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce({
        id: 'db-created-id',
        name: 'huggingface',
        config: {
          isActive: true,
          transport: {
            type: 'stdio',
            command: 'npx',
            args: ['-y', '@fre4x/huggingface'],
          },
        },
        toolCount: null,
        verificationStatus: 'pending',
        lastVerificationError: null,
        createdAt: Date.now(),
        updatedAt: Date.now(),
      });

    const result = await upsertMCPServer(createServer());

    expect(safeInvoke).toHaveBeenNthCalledWith(1, 'list_mcp_server_configs');
    expect(safeInvoke).toHaveBeenNthCalledWith(2, 'create_mcp_server_config', {
      name: 'huggingface',
      config: {
        isActive: true,
        transport: {
          type: 'stdio',
          command: 'npx',
          args: ['-y', '@fre4x/huggingface'],
          env: {
            HF_ALLOW_REMOTE_MODELS: 'true',
          },
        },
      },
    });
    expect(result.id).toBe('db-created-id');
  });

  it('updates the existing server using the persisted DB id', async () => {
    vi.mocked(safeInvoke)
      .mockResolvedValueOnce([
        {
          id: 'db-existing-id',
          name: 'huggingface',
          config: {
            isActive: false,
            transport: {
              type: 'stdio',
              command: 'npx',
              args: ['-y', '@fre4x/huggingface'],
            },
          },
          toolCount: 3,
          verificationStatus: 'success',
          lastVerificationError: null,
          createdAt: Date.now() - 1000,
          updatedAt: Date.now() - 1000,
        },
      ])
      .mockResolvedValueOnce({
        id: 'db-existing-id',
        name: 'huggingface',
        config: {
          isActive: true,
          transport: {
            type: 'stdio',
            command: 'npx',
            args: ['-y', '@fre4x/huggingface'],
            env: {
              HF_ALLOW_REMOTE_MODELS: 'true',
            },
          },
        },
        toolCount: null,
        verificationStatus: 'pending',
        lastVerificationError: null,
        createdAt: Date.now() - 1000,
        updatedAt: Date.now(),
      });

    await upsertMCPServer(createServer({ id: 'temp-form-id' }));

    expect(safeInvoke).toHaveBeenNthCalledWith(1, 'list_mcp_server_configs');
    expect(safeInvoke).toHaveBeenNthCalledWith(2, 'update_mcp_server_config', {
      id: 'db-existing-id',
      name: 'huggingface',
      config: {
        isActive: true,
        transport: {
          type: 'stdio',
          command: 'npx',
          args: ['-y', '@fre4x/huggingface'],
          env: {
            HF_ALLOW_REMOTE_MODELS: 'true',
          },
        },
      },
    });
  });
});
