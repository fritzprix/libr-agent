import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  listBuiltinServers,
  listBuiltinTools,
  listBuiltinServersWithMetadata,
  listAvailableBuiltinServerDefinitions,
} from './builtin-tools';
import { safeInvoke } from './core';

vi.mock('./core', () => ({
  safeInvoke: vi.fn(),
}));

describe('backend/builtin-tools', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should list builtin servers via list_builtin_servers', async () => {
    vi.mocked(safeInvoke).mockResolvedValueOnce(['server1', 'server2']);
    const res = await listBuiltinServers();
    expect(safeInvoke).toHaveBeenCalledWith('list_builtin_servers');
    expect(res).toEqual(['server1', 'server2']);
  });

  it('should list builtin tools without server name', async () => {
    vi.mocked(safeInvoke).mockResolvedValueOnce([{ name: 'tool1' }]);
    const res = await listBuiltinTools();
    expect(safeInvoke).toHaveBeenCalledWith('list_builtin_tools', undefined);
    expect(res).toEqual([{ name: 'tool1' }]);
  });

  it('should list builtin tools with server name', async () => {
    vi.mocked(safeInvoke).mockResolvedValueOnce([{ name: 'tool1' }]);
    const res = await listBuiltinTools('server1');
    expect(safeInvoke).toHaveBeenCalledWith('list_builtin_tools', { serverName: 'server1' });
    expect(res).toEqual([{ name: 'tool1' }]);
  });

  it('should list builtin servers with metadata via list_builtin_servers_with_metadata', async () => {
    const mockMeta = [{ name: 'server1', meta: {} }];
    vi.mocked(safeInvoke).mockResolvedValueOnce(mockMeta);
    const res = await listBuiltinServersWithMetadata();
    expect(safeInvoke).toHaveBeenCalledWith('list_builtin_servers_with_metadata');
    expect(res).toEqual(mockMeta);
  });

  it('should list available builtin server definitions via list_available_builtin_server_definitions', async () => {
    const mockDefs = [{ name: 'server1', meta: {} }];
    vi.mocked(safeInvoke).mockResolvedValueOnce(mockDefs);
    const res = await listAvailableBuiltinServerDefinitions();
    expect(safeInvoke).toHaveBeenCalledWith('list_available_builtin_server_definitions');
    expect(res).toEqual(mockDefs);
  });
});
