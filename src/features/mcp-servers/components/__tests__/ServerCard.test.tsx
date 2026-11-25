import { render, screen, fireEvent } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { describe, it, expect, vi } from 'vitest';
import '@testing-library/jest-dom';
import { ServerCard } from '../ServerCard';
import type { MCPServerEntity } from '@/models/chat';
import type { ReactElement } from 'react';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (_key: string, defaultValue: string | Record<string, unknown>) => {
      if (typeof defaultValue === 'string') return defaultValue;
      return String(defaultValue?.defaultValue ?? _key);
    },
  }),
}));

vi.mock('@/lib/backend/assistants', () => ({
  listAssistants: vi.fn().mockResolvedValue([]),
}));

function renderCard(ui: ReactElement) {
  return render(<MemoryRouter>{ui}</MemoryRouter>);
}

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
    renderCard(
      <ServerCard
        server={baseServer}
        onEdit={noop}
        onDelete={noop}
        onToggleActive={noop}
      />,
    );
    expect(screen.getByText('test-server')).toBeInTheDocument();
  });

  it('shows the Tools button disabled when toolCount is null/undefined (#3000 G3.1)', () => {
    renderCard(
      <ServerCard
        server={{ ...baseServer, toolCount: undefined }}
        onEdit={noop}
        onDelete={noop}
        onToggleActive={noop}
      />,
    );
    expect(screen.getByText('Tools')).toBeDisabled();
  });

  it('shows the Tools button disabled when toolCount is 0 (#3000 G3.1)', () => {
    renderCard(
      <ServerCard
        server={{ ...baseServer, toolCount: 0 }}
        onEdit={noop}
        onDelete={noop}
        onToggleActive={noop}
      />,
    );
    expect(screen.getByText('Tools')).toBeDisabled();
  });

  it('shows Browse Tools button enabled when toolCount > 0', () => {
    renderCard(
      <ServerCard
        server={{ ...baseServer, toolCount: 5 }}
        onEdit={noop}
        onDelete={noop}
        onToggleActive={noop}
      />,
    );
    expect(screen.getByText('Tools')).toBeInTheDocument();
    expect(screen.getByText('Tools')).toBeEnabled();
  });

  it('opens ServerToolsModal when Browse Tools is clicked', () => {
    renderCard(
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
    renderCard(
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
    renderCard(
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
    renderCard(
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

  it('renders verification error details on the card', () => {
    renderCard(
      <ServerCard
        server={{
          ...baseServer,
          verificationStatus: 'error',
          lastVerificationError:
            "Failed to connect to 'hn': No such file or directory (os error 2)",
        }}
        onEdit={noop}
        onDelete={noop}
        onToggleActive={noop}
      />,
    );

    expect(screen.getByText('Connection failed')).toBeInTheDocument();
    expect(
      screen.getByText('A required command was not found on this machine.'),
    ).toBeInTheDocument();
    expect(screen.getByText('Open App Wizard')).toBeInTheDocument();

    // #3000 G3.2: Retry sits next to the failed status line, not in the
    // action row, so the row keeps the same Tools / Edit / Delete slots.
    const failedStatus = screen.getByText('Connection failed');
    const retryButton = screen.getByText('Retry').closest('button')!;
    expect(failedStatus.parentElement).toBe(retryButton.parentElement);

    const deleteButton = screen.getByText('Delete').closest('button')!;
    const actionRow = deleteButton.parentElement!;
    expect(actionRow).not.toContainElement(retryButton);
    const buttonsInActionRow = Array.from(
      actionRow.querySelectorAll('button'),
    ).map((b) => b.textContent?.trim());
    expect(buttonsInActionRow).toEqual(['Tools', 'Edit', 'Delete']);
  });

  it('shows verifying state for pending verification', () => {
    renderCard(
      <ServerCard
        server={{
          ...baseServer,
          verificationStatus: 'pending',
        }}
        onEdit={noop}
        onDelete={noop}
        onToggleActive={noop}
      />,
    );

    expect(screen.getAllByText('Verifying...').length).toBeGreaterThan(0);
    expect(screen.getByText(/Starting extension process/i)).toBeInTheDocument();
  });

  it('wraps long HTTP transport URLs with break-all', () => {
    renderCard(
      <ServerCard
        server={{
          ...baseServer,
          transport: {
            type: 'http-sse',
            url: 'https://mcp.exa.ai/mcp?tools=web_search_advanced_exa%2Cweb_search_exa',
          },
        }}
        onEdit={noop}
        onDelete={noop}
        onToggleActive={noop}
      />,
    );

    const transportLine = screen.getByText(
      /Transport: http-sse • https:\/\/mcp\.exa\.ai\/mcp/,
    );
    expect(transportLine).toHaveClass('break-all');
    expect(transportLine).toHaveTextContent(
      'Transport: http-sse • https://mcp.exa.ai/mcp?tools=web_search_advanced_exa%2Cweb_search_exa',
    );
  });

  it('omits url-param secrets from the displayed HTTP URL', () => {
    renderCard(
      <ServerCard
        server={{
          ...baseServer,
          transport: {
            type: 'http-sse',
            url: 'https://mcp.exa.ai/mcp?tools=web_search_exa&api_key=secret',
          },
          metadata: {
            variableDefinitions: {
              api_key: { target: 'url-param' },
            },
          },
        }}
        onEdit={noop}
        onDelete={noop}
        onToggleActive={noop}
      />,
    );

    expect(
      screen.getByText(
        'Transport: http-sse • https://mcp.exa.ai/mcp?tools=web_search_exa',
      ),
    ).toBeInTheDocument();
    expect(screen.queryByText(/api_key=secret/)).not.toBeInTheDocument();
  });

  // #2924 (D2, gatekeeper G1.4): plugin-owned servers are read-only in the
  // UI — they show a badge and their Edit/Delete actions are disabled.
  describe('plugin-owned servers (#2924)', () => {
    const pluginServer: MCPServerEntity = {
      ...baseServer,
      pluginSource: 'plugin:plugin-0123456789abcdef',
    };

    it('shows the Plugin badge for plugin-owned servers', () => {
      renderCard(
        <ServerCard
          server={pluginServer}
          onEdit={noop}
          onDelete={noop}
          onToggleActive={noop}
        />,
      );

      expect(screen.getByText('Plugin')).toBeInTheDocument();
    });

    it('disables Edit and Delete and never fires their handlers', () => {
      const onEdit = vi.fn();
      const onDelete = vi.fn();
      renderCard(
        <ServerCard
          server={pluginServer}
          onEdit={onEdit}
          onDelete={onDelete}
          onToggleActive={noop}
        />,
      );

      const editButton = screen.getByText('Edit').closest('button');
      const deleteButton = screen.getByText('Delete').closest('button');
      expect(editButton).toBeDisabled();
      expect(deleteButton).toBeDisabled();

      fireEvent.click(editButton!);
      fireEvent.click(deleteButton!);
      expect(onEdit).not.toHaveBeenCalled();
      expect(onDelete).not.toHaveBeenCalled();
    });

    it('keeps Edit and Delete enabled for normal servers', () => {
      renderCard(
        <ServerCard
          server={baseServer}
          onEdit={noop}
          onDelete={noop}
          onToggleActive={noop}
        />,
      );

      expect(screen.queryByText('Plugin')).not.toBeInTheDocument();
      expect(screen.getByText('Edit').closest('button')).not.toBeDisabled();
      expect(screen.getByText('Delete').closest('button')).not.toBeDisabled();
    });
  });

  describe('card layout and action row alignment', () => {
    it('places Authorize inline next to Authorization Required and keeps Tools, Edit, Delete on one bottom-pinned row', () => {
      const oauthServer: MCPServerEntity = {
        ...baseServer,
        id: 'srv-oauth',
        name: 'remote-mcp',
        toolCount: 4,
        transport: { type: 'http-sse', url: 'https://example.com/sse' },
        authentication: { type: 'oauth2.1' },
      };

      const { container } = renderCard(
        <ServerCard
          server={oauthServer}
          onEdit={noop}
          onDelete={noop}
          onToggleActive={noop}
        />,
      );

      const card = container.firstElementChild as HTMLElement;
      expect(card).toHaveClass('flex', 'h-full', 'flex-col');

      // Header aligns Active toggle to the top consistently
      const header = Array.from(card.children).find((el) =>
        el.contains(screen.getByText('Active')),
      );
      expect(header).not.toBeUndefined();
      expect(header).toHaveClass('items-start', 'gap-2');
      expect(header).not.toHaveClass('items-center');

      // Authorize sits inline next to the "Authorization Required" status line, NOT in the action row
      const authStatus = screen.getByText('Authorization Required');
      const authorizeButton = screen.getByText('Authorize').closest('button')!;
      expect(authStatus.parentElement).toBe(authorizeButton.parentElement);

      // Action row is pinned to bottom and contains Tools, Edit, Delete in one single row
      const deleteButton = screen.getByText('Delete').closest('button')!;
      const actionRow = deleteButton.parentElement!;
      expect(actionRow).toHaveClass('flex', 'items-center', 'justify-between', 'gap-2');
      expect(actionRow).not.toHaveClass('flex-wrap');
      expect(actionRow.parentElement).toHaveClass('mt-auto');
      expect(actionRow).not.toContainElement(authorizeButton);

      const buttonsInActionRow = Array.from(actionRow.querySelectorAll('button')).map((b) =>
        b.textContent?.trim(),
      );
      expect(buttonsInActionRow).toEqual(['Tools', 'Edit', 'Delete']);
    });
  });
});
