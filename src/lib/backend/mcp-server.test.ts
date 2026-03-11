import { describe, it, expect, vi, beforeEach } from 'vitest';
import { safeInvoke } from './core';
import {
  callTool,
  hasOAuthToken,
  getOAuthToken,
  revokeOAuthToken,
  sampleFromModel,
  validateToolSchema,
} from './mcp-server';
import type { MCPTool, SamplingOptions } from '@/lib/mcp';
import * as cuid2 from '@paralleldrive/cuid2';

// Mock the safeInvoke function
vi.mock('./core', () => ({
  safeInvoke: vi.fn(),
}));

// Mock cuid2 to return a predictable ID when createId is called
vi.mock('@paralleldrive/cuid2', () => ({
  createId: vi.fn(() => 'mock-cuid'),
}));

describe('mcp-server backend API', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('callTool', () => {
    it('should call call_mcp_tool with provided requestId', async () => {
      const serverName = 'test-server';
      const toolName = 'test-tool';
      const args = { foo: 'bar' };
      const requestId = 'custom-request-id';
      const mockResponse = { result: { content: [] } };

      vi.mocked(safeInvoke).mockResolvedValueOnce(mockResponse);

      const result = await callTool(serverName, toolName, args, requestId);

      expect(safeInvoke).toHaveBeenCalledTimes(1);
      expect(safeInvoke).toHaveBeenCalledWith('call_mcp_tool', {
        serverName,
        toolName,
        arguments: args,
        requestId,
      });
      expect(result).toEqual(mockResponse);
    });

    it('should generate a requestId if one is not provided', async () => {
      const serverName = 'test-server';
      const toolName = 'test-tool';
      const args = { foo: 'bar' };
      const mockResponse = { result: { content: [] } };

      vi.mocked(safeInvoke).mockResolvedValueOnce(mockResponse);

      const result = await callTool(serverName, toolName, args);

      expect(cuid2.createId).toHaveBeenCalledTimes(1);
      expect(safeInvoke).toHaveBeenCalledTimes(1);
      expect(safeInvoke).toHaveBeenCalledWith('call_mcp_tool', {
        serverName,
        toolName,
        arguments: args,
        requestId: 'mock-cuid',
      });
      expect(result).toEqual(mockResponse);
    });
  });

  describe('hasOAuthToken', () => {
    it('should call has_oauth_token with serverId', async () => {
      vi.mocked(safeInvoke).mockResolvedValueOnce(true);

      const result = await hasOAuthToken('server-123');

      expect(safeInvoke).toHaveBeenCalledTimes(1);
      expect(safeInvoke).toHaveBeenCalledWith('has_oauth_token', {
        serverId: 'server-123',
      });
      expect(result).toBe(true);
    });
  });

  describe('getOAuthToken', () => {
    it('should call get_oauth_token with serverId', async () => {
      vi.mocked(safeInvoke).mockResolvedValueOnce('mock-token');

      const result = await getOAuthToken('server-123');

      expect(safeInvoke).toHaveBeenCalledTimes(1);
      expect(safeInvoke).toHaveBeenCalledWith('get_oauth_token', {
        serverId: 'server-123',
      });
      expect(result).toBe('mock-token');
    });

    it('should handle null return value', async () => {
      vi.mocked(safeInvoke).mockResolvedValueOnce(null);

      const result = await getOAuthToken('server-123');

      expect(safeInvoke).toHaveBeenCalledTimes(1);
      expect(safeInvoke).toHaveBeenCalledWith('get_oauth_token', {
        serverId: 'server-123',
      });
      expect(result).toBeNull();
    });
  });

  describe('revokeOAuthToken', () => {
    it('should call revoke_oauth_token with serverId', async () => {
      vi.mocked(safeInvoke).mockResolvedValueOnce('Token revoked');

      const result = await revokeOAuthToken('server-123');

      expect(safeInvoke).toHaveBeenCalledTimes(1);
      expect(safeInvoke).toHaveBeenCalledWith('revoke_oauth_token', {
        serverId: 'server-123',
      });
      expect(result).toBe('Token revoked');
    });
  });

  describe('sampleFromModel', () => {
    it('should call sample_from_mcp_server with all provided parameters', async () => {
      const serverName = 'test-server';
      const prompt = 'Hello world';
      const options: SamplingOptions = { maxTokens: 100, temperature: 0.5 };
      const mockResponse = { result: { sampling: { finishReason: 'stop' } } };

      vi.mocked(safeInvoke).mockResolvedValueOnce(mockResponse);

      const result = await sampleFromModel(serverName, prompt, options);

      expect(safeInvoke).toHaveBeenCalledTimes(1);
      expect(safeInvoke).toHaveBeenCalledWith('sample_from_mcp_server', {
        serverName,
        prompt,
        options,
      });
      expect(result).toEqual(mockResponse);
    });

    it('should call sample_from_mcp_server without options if not provided', async () => {
      const serverName = 'test-server';
      const prompt = 'Hello world';
      const mockResponse = { result: { sampling: { finishReason: 'stop' } } };

      vi.mocked(safeInvoke).mockResolvedValueOnce(mockResponse);

      const result = await sampleFromModel(serverName, prompt);

      expect(safeInvoke).toHaveBeenCalledTimes(1);
      expect(safeInvoke).toHaveBeenCalledWith('sample_from_mcp_server', {
        serverName,
        prompt,
        options: undefined,
      });
      expect(result).toEqual(mockResponse);
    });
  });

  describe('validateToolSchema', () => {
    it('should call validate_tool_schema with the provided tool', async () => {
      const tool: MCPTool = {
        name: 'test-tool',
        description: 'A test tool',
        inputSchema: { type: 'object' },
      };

      vi.mocked(safeInvoke).mockResolvedValueOnce(undefined);

      await validateToolSchema(tool);

      expect(safeInvoke).toHaveBeenCalledTimes(1);
      expect(safeInvoke).toHaveBeenCalledWith('validate_tool_schema', {
        tool,
      });
    });

    it('should reject if safeInvoke rejects', async () => {
      const tool: MCPTool = {
        name: 'test-tool',
        description: 'A test tool',
        inputSchema: { type: 'object' },
      };

      const error = new Error('Invalid schema');
      vi.mocked(safeInvoke).mockRejectedValueOnce(error);

      await expect(validateToolSchema(tool)).rejects.toThrow('Invalid schema');

      expect(safeInvoke).toHaveBeenCalledTimes(1);
      expect(safeInvoke).toHaveBeenCalledWith('validate_tool_schema', {
        tool,
      });
    });
  });
});
