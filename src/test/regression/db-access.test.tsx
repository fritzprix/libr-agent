import { describe, it, expect, vi } from 'vitest';
import { renderHook, waitFor } from '@testing-library/react';
import { useMCPServerRegistry, MCPServerRegistryProvider } from '@/context/MCPServerRegistryContext';
import { SettingsProvider } from '@/context/SettingsContext';
import { ReactNode } from 'react';

// Mock useWebMCP to avoid Web Worker initialization issues
vi.mock('@/context/WebMCPContext', async (importOriginal) => {
    const actual = await importOriginal<typeof import('@/context/WebMCPContext')>();
    return {
        ...actual,
        useWebMCP: () => ({
            proxy: null,
            isLoading: false,
            initialized: true,
            getServerProxy: vi.fn(),
            switchServerContext: vi.fn(),
        }),
        WebMCPProvider: ({ children }: { children: ReactNode }) => <>{children}</>,
    };
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

        await result.current.saveServer(newServer);

        await waitFor(() => {
            expect(result.current.allServers.some((s) => s.id === 'test-server')).toBe(
                true,
            );
        });
    });

    it('should delete MCP servers via service layer', async () => {
        const { result } = renderHook(() => useMCPServerRegistry(), { wrapper });

        await result.current.deleteServer('test-server');

        await waitFor(() => {
            expect(result.current.allServers.some((s) => s.id === 'test-server')).toBe(
                false,
            );
        });
    });
});
