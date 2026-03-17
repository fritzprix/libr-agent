import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  listBuiltinServers,
  listBuiltinTools,
  listBuiltinServersWithMetadata,
  listAvailableBuiltinServerDefinitions
} from '../builtin-tools';
import { safeInvoke } from '../core';

vi.mock('../core', () => ({
  safeInvoke: vi.fn(),
}));

describe('builtin-tools backend commands', () => {
  beforeEach(() => {
    vi.mocked(safeInvoke).mockClear();
  });

  it('listBuiltinServers calls correct backend command', async () => {
    const mockServers = ['server1', 'server2'];
    vi.mocked(safeInvoke).mockResolvedValueOnce(mockServers);

    const result = await listBuiltinServers();

    expect(safeInvoke).toHaveBeenCalledWith('list_builtin_servers');
    expect(result).toBe(mockServers);
  });

  it('listBuiltinTools calls correct backend command without serverName', async () => {
    const mockTools = [{ name: 'tool1' }];
    vi.mocked(safeInvoke).mockResolvedValueOnce(mockTools);

    const result = await listBuiltinTools();

    expect(safeInvoke).toHaveBeenCalledWith('list_builtin_tools', undefined);
    expect(result).toBe(mockTools);
  });

  it('listBuiltinTools calls correct backend command with serverName', async () => {
    const mockTools = [{ name: 'tool1' }];
    vi.mocked(safeInvoke).mockResolvedValueOnce(mockTools);

    const result = await listBuiltinTools('server1');

    expect(safeInvoke).toHaveBeenCalledWith('list_builtin_tools', { serverName: 'server1' });
    expect(result).toBe(mockTools);
  });

  it('listBuiltinServersWithMetadata calls correct backend command', async () => {
    const mockServers = [{ name: 'server1', toolsCount: 1 }];
    vi.mocked(safeInvoke).mockResolvedValueOnce(mockServers);

    const result = await listBuiltinServersWithMetadata();

    expect(safeInvoke).toHaveBeenCalledWith('list_builtin_servers_with_metadata');
    expect(result).toBe(mockServers);
  });

  it('listAvailableBuiltinServerDefinitions calls correct backend command', async () => {
    const mockServers = [{ name: 'server1', toolsCount: 1 }];
    vi.mocked(safeInvoke).mockResolvedValueOnce(mockServers);

    const result = await listAvailableBuiltinServerDefinitions();

    expect(safeInvoke).toHaveBeenCalledWith('list_available_builtin_server_definitions');
    expect(result).toBe(mockServers);
  });
});
