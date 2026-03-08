import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import '@testing-library/jest-dom';
import { ServerCard } from '../ServerCard';
import type { MCPServerEntity } from '@/models/chat';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (_key: string, defaultValue: string | Record<string, unknown>) => {
      if (typeof defaultValue === 'string') return defaultValue;
      return String(defaultValue?.defaultValue ?? _key);
    },
  }),
}));

// Mock ServerToolsModal to avoid safeInvoke() calls in tests
vi.mock('../ServerToolsModal', () => ({
  ServerToolsModal: ({
    isOpen,
    onClose,
    serverName,
  }: {
    isOpen: boolean;
    onClose: () => void;
    serverName: string;
  }) =>
    isOpen ? (
      <div data-testid="tools-modal">
        <span>{serverName}</span>
        <button onClick={onClose}>Close</button>
      </div>
    ) : null,
}));

global.ResizeObserver = class ResizeObserver {
  observe() {}
  unobserve() {}
  disconnect() {}
};

const baseServer: MCPServerEntity = {
  id: 'srv-001',
  name: 'test-server',
  isActive: true,
  transport: { type: 'stdio', command: 'npx', args: [], env: {} },
  createdAt: new Date(),
  updatedAt: new Date(),
};

const noop = () => {};

describe('ServerCard', () => {
  it('renders server name', () => {
    render(
      <ServerCard
        server={baseServer}
        onEdit={noop}
        onDelete={noop}
        onToggleActive={noop}
      />,
    );
    expect(screen.getByText('test-server')).toBeInTheDocument();
  });

  it('does NOT show Browse Tools button when toolCount is null/undefined', () => {
    render(
      <ServerCard
        server={{ ...baseServer, toolCount: undefined }}
        onEdit={noop}
        onDelete={noop}
        onToggleActive={noop}
      />,
    );
    expect(screen.queryByText('Tools')).not.toBeInTheDocument();
  });

  it('does NOT show Browse Tools button when toolCount is 0', () => {
    render(
      <ServerCard
        server={{ ...baseServer, toolCount: 0 }}
        onEdit={noop}
        onDelete={noop}
        onToggleActive={noop}
      />,
    );
    expect(screen.queryByText('Tools')).not.toBeInTheDocument();
  });

  it('shows Browse Tools button when toolCount > 0', () => {
    render(
      <ServerCard
        server={{ ...baseServer, toolCount: 5 }}
        onEdit={noop}
        onDelete={noop}
        onToggleActive={noop}
      />,
    );
    expect(screen.getByText('Tools')).toBeInTheDocument();
  });

  it('opens ServerToolsModal when Browse Tools is clicked', () => {
    render(
      <ServerCard
        server={{ ...baseServer, toolCount: 3 }}
        onEdit={noop}
        onDelete={noop}
        onToggleActive={noop}
      />,
    );
    expect(screen.queryByTestId('tools-modal')).not.toBeInTheDocument();
    fireEvent.click(screen.getByText('Tools'));
    expect(screen.getByTestId('tools-modal')).toBeInTheDocument();
  });

  it('closes ServerToolsModal when onClose is called', () => {
    render(
      <ServerCard
        server={{ ...baseServer, toolCount: 3 }}
        onEdit={noop}
        onDelete={noop}
        onToggleActive={noop}
      />,
    );
    fireEvent.click(screen.getByText('Tools'));
    expect(screen.getByTestId('tools-modal')).toBeInTheDocument();
    fireEvent.click(screen.getByText('Close'));
    expect(screen.queryByTestId('tools-modal')).not.toBeInTheDocument();
  });

  it('calls onEdit when Edit button is clicked', () => {
    const onEdit = vi.fn();
    render(
      <ServerCard
        server={baseServer}
        onEdit={onEdit}
        onDelete={noop}
        onToggleActive={noop}
      />,
    );
    fireEvent.click(screen.getByText('Edit'));
    expect(onEdit).toHaveBeenCalledWith(baseServer);
  });

  it('calls onDelete when Delete button is clicked', () => {
    const onDelete = vi.fn();
    render(
      <ServerCard
        server={baseServer}
        onEdit={noop}
        onDelete={onDelete}
        onToggleActive={noop}
      />,
    );
    fireEvent.click(screen.getByText('Delete'));
    expect(onDelete).toHaveBeenCalledWith(baseServer);
  });
});
