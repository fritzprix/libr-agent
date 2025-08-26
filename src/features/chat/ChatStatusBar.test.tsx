import { render, screen } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import Chat from './Chat'; // Assuming ChatStatusBar is exported from here or accessible
import {
  MCPServerContext,
  MCPServerContextType,
} from '@/context/MCPServerContext';
import { ReactNode } from 'react';
import { MCPTool } from '@/lib/mcp-types';

// Mock child components that are not relevant to this test
vi.mock('@/components/ui', async (importOriginal) => {
  const original = await importOriginal<typeof import('@/components/ui')>();
  return {
    ...original,
    CompactModelPicker: () => <div>CompactModelPicker</div>,
  };
});

const mockTools: MCPTool[] = [
  {
    name: 'tool1',
    description: 'd1',
    inputSchema: { type: 'object', properties: {} },
  },
];

// Custom provider for testing
const MockMCPServerProvider = ({
  children,
  value,
}: {
  children: ReactNode;
  value: Partial<MCPServerContextType>;
}) => {
  const defaultValue: MCPServerContextType = {
    availableTools: [],
    toolsByServer: {},
    isLoading: false,
    error: undefined,
    status: {},
    getAvailableTools: () => [],
    connectServers: async () => {},
    executeToolCall: async () => ({
      jsonrpc: '2.0' as const,
      id: 'test',
      result: { content: [] },
    }),
    sampleFromModel: async () => ({
      jsonrpc: '2.0' as const,
      id: 'test',
      result: { content: [] },
    }),
  };

  return (
    <MCPServerContext.Provider value={{ ...defaultValue, ...value }}>
      {children}
    </MCPServerContext.Provider>
  );
};

describe('Chat.StatusBar', () => {
  it('should display loading state', () => {
    render(
      <MockMCPServerProvider value={{ isLoading: true }}>
        <Chat.StatusBar />
      </MockMCPServerProvider>,
    );

    expect(screen.getByText('Loading tools...')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /loading/i })).toBeDisabled();
  });

  it('should display error state', () => {
    const errorMessage = 'Failed to connect';
    render(
      <MockMCPServerProvider value={{ error: errorMessage }}>
        <Chat.StatusBar />
      </MockMCPServerProvider>,
    );

    expect(screen.getByText(/Tools error/i)).toBeInTheDocument();
    const button = screen.getByRole('button', { name: /error/i });
    expect(button).not.toBeDisabled();
    expect(button).toHaveAttribute('title', errorMessage);
    expect(button.textContent).toContain('⚠️');
  });

  it('should display success state with tools', () => {
    render(
      <MockMCPServerProvider
        value={{
          availableTools: mockTools,
          isLoading: false,
          error: undefined,
        }}
      >
        <Chat.StatusBar />
      </MockMCPServerProvider>,
    );

    expect(screen.getByText(/1 available/i)).toBeInTheDocument();
    const button = screen.getByRole('button', { name: /available/i });
    expect(button).not.toBeDisabled();
    expect(button.textContent).toContain('🔧');
  });

  it('should display success state with zero tools', () => {
    render(
      <MockMCPServerProvider
        value={{ availableTools: [], isLoading: false, error: undefined }}
      >
        <Chat.StatusBar />
      </MockMCPServerProvider>,
    );

    expect(screen.getByText(/0 available/i)).toBeInTheDocument();
    const button = screen.getByRole('button', { name: /available/i });
    expect(button).not.toBeDisabled();
    expect(button.textContent).toContain('🔧');
  });
});
