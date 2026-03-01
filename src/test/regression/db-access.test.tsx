import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, waitFor, act } from '@testing-library/react';
import { useMCPServerRegistry, MCPServerRegistryProvider } from '@/context/MCPServerRegistryContext';
import { SettingsProvider } from '@/context/SettingsContext';
import { ReactNode } from 'react';

// Stateful mock stores
const mockServerStore = new Map<string, { name: string; config: unknown; createdAt: number; updatedAt: number }>();
const mockAssistantStore: unknown[] = [];

// Mock Tauri API first (before any module imports)
vi.mock('@/lib/backend/core', async () => {
    const mockInvoke = vi.fn().mockImplementation(async (cmd: string, args?: Record<string, unknown>) => {
        // Return proper DTOs for MCP server operations
        if (cmd === 'create_mcp_server_config' || cmd === 'update_mcp_server_config') {
            const id = (args?.id as string) || (args?.name as string) || 'test-server';
            const name = (args?.name as string) || id;
            const config = args?.config || {};
            const dto = {
                id,
                name,
                config,
                createdAt: Date.now(),
                updatedAt: Date.now(),
            };
            mockServerStore.set(id, dto);
            return dto;
        }
        if (cmd === 'list_mcp_server_configs') {
            return Array.from(mockServerStore.values());
        }
        if (cmd === 'delete_mcp_server_config') {
            const id = (args?.id as string) || (args?.name as string) || 'test-server';
            mockServerStore.delete(id);
            return undefined;
        }
        if (cmd === 'list_assistants') {
            return mockAssistantStore;
        }
        return undefined;
    });

    return {
        safeInvoke: mockInvoke,
        invoke: mockInvoke,
    };
});

// Mock GlobalEventContext
vi.mock('@/context/GlobalEventContext', () => ({
    useBackendResource: vi.fn(),
}));

// Clear mock store before each test
beforeEach(() => {
    mockServerStore.clear();
    mockAssistantStore.length = 0;
});

// Mock wrapper
const wrapper = ({ children }: { children: ReactNode }) => (
    <SettingsProvider>
        <MCPServerRegistryProvider>{children}</MCPServerRegistryProvider>
    </SettingsProvider>
);

describe('DB Access Regression Tests', () => {
    it('should save and load MCP servers via service layer', async () => {
        const { result } = renderHook(() => useMCPServerRegistry(), { wrapper });

        // Initial load should be empty
        expect(result.current.allServers).toHaveLength(0);

        const newServer = {
            id: 'test-server', // Will be overwritten by backend using 'name' as ID
            name: 'Test Server',
            transport: {
                type: 'stdio' as const,
                command: 'node',
                args: ['server.js'],
            },
            isActive: true,
            createdAt: new Date(),
            updatedAt: new Date(),
        };

        // Save the server - this should trigger refreshAll via revalidation event
        await act(async () => {
            await result.current.saveServer(newServer);
        });

        // After save, the context should have emitted a revalidation event
        // and called refreshAll, which should populate allServers
        // Wait longer for async operations
        await new Promise((resolve) => setTimeout(resolve, 200));

        // Note: Backend uses 'name' as ID, not the ID field
        await waitFor(
            () => {
                expect(result.current.allServers.some((s) => s.id === 'Test Server')).toBe(
                    true,
                );
            },
            { timeout: 2000 },
        );
    });

    it('should delete MCP servers via service layer', async () => {
        const { result } = renderHook(() => useMCPServerRegistry(), { wrapper });

        // First, save a server
        const newServer = {
            id: 'test-server',
            name: 'Test Server',
            transport: {
                type: 'stdio' as const,
                command: 'node',
                args: ['server.js'],
            },
            isActive: true,
            createdAt: new Date(),
            updatedAt: new Date(),
        };

        await act(async () => {
            await result.current.saveServer(newServer);
        });

        await waitFor(
            () => {
                expect(result.current.allServers.some((s) => s.id === 'Test Server')).toBe(
                    true,
                );
            },
            { timeout: 2000 },
        );

        // Now delete it - use the name since backend uses name as ID
        await act(async () => {
            await result.current.deleteServer('Test Server');
        });

        await waitFor(
            () => {
                expect(result.current.allServers.some((s) => s.id === 'Test Server')).toBe(
                    false,
                );
            },
            { timeout: 2000 },
        );
    });
});
