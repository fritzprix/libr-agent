import { render, screen, act } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { MCPServerProvider, MCPServerContext } from './MCPServerContext';
import { tauriMCPClient } from '../lib/tauri-mcp-client';
import React from 'react';
import { MCPTool } from '../lib/mcp-types';
import { MCPConfig } from '../models/chat';

// Mock the tauriMCPClient
vi.mock('../lib/tauri-mcp-client', () => ({
  tauriMCPClient: {
    listToolsFromConfig: vi.fn(),
    getConnectedServers: vi.fn(),
  },
}));

// Mock the logger
vi.mock('../lib/logger', () => ({
  getLogger: () => ({
    debug: vi.fn(),
    error: vi.fn(),
    info: vi.fn(),
    warn: vi.fn(),
  }),
}));

const mockTools: MCPTool[] = [
  {
    name: 'tool1',
    description: 'description1',
    inputSchema: { type: 'object', properties: {} },
  },
  {
    name: 'tool2',
    description: 'description2',
    inputSchema: { type: 'object', properties: {} },
  },
];

const TestConsumer = () => {
  const context = React.useContext(MCPServerContext);
  if (!context) return null;

  return (
    <div>
      <span>Loading: {String(context.isLoading)}</span>
      <span>Error: {context.error || 'none'}</span>
      <span>Tools: {context.availableTools.length}</span>
    </div>
  );
};

describe('MCPServerProvider', () => {
  beforeEach(() => {
    vi.resetAllMocks();
  });

  it('should have correct initial state', () => {
    render(
      <MCPServerProvider>
        <TestConsumer />
      </MCPServerProvider>,
    );

    expect(screen.getByText(/Loading: false/)).toBeInTheDocument();
    expect(screen.getByText(/Error: none/)).toBeInTheDocument();
    expect(screen.getByText(/Tools: 0/)).toBeInTheDocument();
  });

  it('should handle successful server connection', async () => {
    (
      tauriMCPClient.listToolsFromConfig as ReturnType<typeof vi.fn>
    ).mockResolvedValue(mockTools);
    (
      tauriMCPClient.getConnectedServers as ReturnType<typeof vi.fn>
    ).mockResolvedValue(['server1']);

    let connectServers: (config: MCPConfig) => Promise<void>;

    const MCPServerController = () => {
      const context = React.useContext(MCPServerContext);
      connectServers = context!.connectServers;
      return <TestConsumer />;
    };

    render(
      <MCPServerProvider>
        <MCPServerController />
      </MCPServerProvider>,
    );

    await act(async () => {
      await connectServers({
        mcpServers: { server1: { command: 'test-command', args: [] } },
      });
    });

    expect(screen.getByText(/Loading: false/)).toBeInTheDocument();
    expect(screen.getByText(/Error: none/)).toBeInTheDocument();
    expect(screen.getByText(`Tools: ${mockTools.length}`)).toBeInTheDocument();
  });

  it('should handle failed server connection', async () => {
    const errorMessage = 'Connection failed';
    (
      tauriMCPClient.listToolsFromConfig as ReturnType<typeof vi.fn>
    ).mockRejectedValue(new Error(errorMessage));

    let connectServers: (config: MCPConfig) => Promise<void>;

    const MCPServerController = () => {
      const context = React.useContext(MCPServerContext);
      connectServers = context!.connectServers;
      return <TestConsumer />;
    };

    render(
      <MCPServerProvider>
        <MCPServerController />
      </MCPServerProvider>,
    );

    await act(async () => {
      await connectServers({
        mcpServers: { server1: { command: 'test-command', args: [] } },
      });
    });

    expect(screen.getByText(/Loading: false/)).toBeInTheDocument();
    expect(screen.getByText(`Error: ${errorMessage}`)).toBeInTheDocument();
    expect(screen.getByText(/Tools: 0/)).toBeInTheDocument();
  });
});
