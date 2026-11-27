import {
  act,
  render,
  screen,
  fireEvent,
} from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { beforeEach, describe, it, expect, vi } from 'vitest';
import '@testing-library/jest-dom';
import { ServerCard } from '../ServerCard';
import { __resetAuthFlowsForTest } from '../../authFlowStore';
import {
  cancelOAuthFlow,
  takeOAuthFlowLastOutcome,
  hasOAuthToken,
  isOAuthFlowActive,
  revokeOAuthToken,
  startOAuthFlow,
} from '@/lib/backend';
import { listen } from '@tauri-apps/api/event';
import { safeInvoke } from '@/lib/backend/core';
import type { MCPServerEntity } from '@/models/chat';
import type { ReactElement } from 'react';

type FlowEndedHandler = (event: {
  payload: {
    serverId: string;
    success: boolean;
    message?: string;
  };
}) => void;

vi.mock('react-i18next', async () => {
  // Minimal i18next stand-in backed by the REAL en locale file, so tests
  // exercise production locale strings and {{interpolation}} (#3040 G2.1).
  const en = (await import('@/locales/en/common.json')).default;
  const lookup = (key: string): unknown =>
    key
      .split('.')
      .reduce<unknown>(
        (obj, part) =>
          obj && typeof obj === 'object'
            ? (obj as Record<string, unknown>)[part]
            : undefined,
        en,
      );
  return {
    useTranslation: () => ({
      t: (key: string, second?: unknown) => {
        const raw = lookup(key);
        const opts =
          typeof second === 'object' && second !== null
            ? (second as Record<string, unknown>)
            : {};
        const fallback =
          typeof second === 'string'
            ? second
            : typeof opts.defaultValue === 'string'
              ? opts.defaultValue
              : key;
        const template = typeof raw === 'string' ? raw : fallback;
        return template.replace(
          /\{\{\s*(\w+)\s*\}\}/g,
          (_match, variable: string) => String(opts[variable] ?? ''),
        );
      },
    }),
  };
});

vi.mock('@/lib/backend/assistants', () => ({
  listAssistants: vi.fn().mockResolvedValue([]),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async () => () => undefined),
}));

vi.mock('@/lib/backend', () => ({
  hasOAuthToken: vi.fn().mockResolvedValue(false),
  startOAuthFlow: vi.fn(
    () => new Promise(() => {
      /* leave authorizing until cancelled */
    }),
  ),
  cancelOAuthFlow: vi.fn().mockResolvedValue('cancelled'),
  revokeOAuthToken: vi.fn().mockResolvedValue('revoked'),
  isOAuthFlowActive: vi.fn().mockResolvedValue(false),
  takeOAuthFlowLastOutcome: vi.fn().mockResolvedValue(null),
}));

vi.mock('@/lib/backend/core', () => ({
  safeInvoke: vi.fn().mockResolvedValue([]),
}));

vi.mock('sonner', () => ({
  toast: { success: vi.fn(), error: vi.fn(), message: vi.fn() },
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

  it('shows the Tools button disabled with an explanatory title when toolCount is 0 (#3000 G3.1/S4.1)', () => {
    renderCard(
      <ServerCard
        server={{ ...baseServer, toolCount: 0 }}
        onEdit={noop}
        onDelete={noop}
        onToggleActive={noop}
      />,
    );
    const tools = screen.getByText('Tools').closest('button');
    expect(tools).toBeDisabled();
    // S4.1: a disabled button must say why, like the plugin-owned Edit/Delete.
    expect(tools).toHaveAttribute(
      'title',
      'Tool count unknown (not yet verified)',
    );
  });

  it('shows Browse Tools button enabled and untitled when toolCount > 0', () => {
    renderCard(
      <ServerCard
        server={{ ...baseServer, toolCount: 5 }}
        onEdit={noop}
        onDelete={noop}
        onToggleActive={noop}
      />,
    );
    const tools = screen.getByText('Tools').closest('button');
    expect(tools).toBeInTheDocument();
    expect(tools).toBeEnabled();
    expect(tools).not.toHaveAttribute('title');
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
    // G1.3/G1.4: both share one row that never wraps; the status text
    // truncates (with a title tooltip) while Retry keeps its full size.
    const failedStatus = screen.getByText('Connection failed');
    const retryButton = screen.getByText('Retry').closest('button')!;
    const failedRow = failedStatus.closest('div')!;
    expect(failedRow).toContainElement(retryButton);
    expect(failedRow).not.toHaveClass('flex-wrap');
    expect(failedStatus).toHaveClass('truncate');
    expect(failedStatus).toHaveAttribute('title', 'Connection failed');
    expect(retryButton).toHaveClass('shrink-0');

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

      // Authorize sits inline next to the "Authorization Required" status line, NOT in the action row.
      // G1.1/G1.2/G1.4: one non-wrapping row; the status text truncates
      // (with a title tooltip) so Authorize is always fully visible.
      const authStatus = screen.getByText('Authorization Required');
      const authorizeButton = screen.getByText('Authorize').closest('button')!;
      const authRow = authStatus.closest('div')!;
      expect(authRow).toContainElement(authorizeButton);
      expect(authRow).not.toHaveClass('flex-wrap');
      expect(authStatus).toHaveClass('truncate');
      expect(authStatus).toHaveAttribute('title', 'Authorization Required');
      expect(authorizeButton).toHaveClass('shrink-0');

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

  // #3040: while Authorize is in progress the card shows Cancel.
  describe('OAuth authorize cancel (#3040)', () => {
    const oauthServer: MCPServerEntity = {
      ...baseServer,
      authentication: {
        type: 'oauth2.1',
        authorizationEndpoint: 'https://auth.example.com/authorize',
        tokenEndpoint: 'https://auth.example.com/token',
        clientId: 'client',
        redirectUri: 'http://localhost:14207/callback',
        usePKCE: true,
      },
    };

    const renderOauthCard = () =>
      renderCard(
        <ServerCard
          server={oauthServer}
          onEdit={noop}
          onDelete={noop}
          onToggleActive={noop}
        />,
      );

    let flowEndedHandler: FlowEndedHandler | null = null;

    beforeEach(() => {
      __resetAuthFlowsForTest();
      vi.clearAllMocks();
      flowEndedHandler = null;
      vi.mocked(listen).mockImplementation(async (event, handler) => {
        if (event === 'mcp-oauth-flow-ended') {
          flowEndedHandler = handler as FlowEndedHandler;
        }
        return () => undefined;
      });
      vi.mocked(hasOAuthToken).mockResolvedValue(false);
      vi.mocked(isOAuthFlowActive).mockResolvedValue(false);
      vi.mocked(takeOAuthFlowLastOutcome).mockResolvedValue(null);
      vi.mocked(cancelOAuthFlow).mockResolvedValue('cancelled');
      vi.mocked(revokeOAuthToken).mockResolvedValue('revoked');
      vi.mocked(safeInvoke).mockResolvedValue([]);
      vi.mocked(startOAuthFlow).mockImplementation(
        () =>
          new Promise<string>(() => {
            /* stay pending until the test settles it */
          }),
      );
    });

    async function emitFlowEnded(payload: {
      serverId: string;
      success: boolean;
      message?: string;
    }) {
      await act(async () => {
        // Listener registers asynchronously from ensureOAuthFlowEndedListener.
        await vi.waitFor(() => expect(flowEndedHandler).not.toBeNull());
        flowEndedHandler!({ payload });
      });
    }

    it('shows Cancel while authorization is in progress and invokes cancel', async () => {
      renderOauthCard();

      fireEvent.click(screen.getByTestId('oauth-authorize'));
      expect(await screen.findByTestId('oauth-cancel')).toBeInTheDocument();
      expect(startOAuthFlow).toHaveBeenCalledWith(
        oauthServer.id,
        oauthServer.authentication,
      );

      fireEvent.click(screen.getByTestId('oauth-cancel'));
      expect(cancelOAuthFlow).toHaveBeenCalledWith(oauthServer.id);
    });

    it('interpolates the cause into the real authCancelFailed locale string (#3040 G2.1)', async () => {
      vi.mocked(cancelOAuthFlow).mockRejectedValueOnce(
        new Error('no matching OAuth authorization is in progress'),
      );
      renderOauthCard();

      fireEvent.click(screen.getByTestId('oauth-authorize'));
      expect(await screen.findByTestId('oauth-cancel')).toBeInTheDocument();
      fireEvent.click(screen.getByTestId('oauth-cancel'));

      const toast = await import('sonner');
      await vi.waitFor(() =>
        expect(toast.toast.error).toHaveBeenCalledWith(
          'Could not cancel: no matching OAuth authorization is in progress',
        ),
      );
      expect(toast.toast.error).not.toHaveBeenCalledWith(
        expect.stringContaining('{{error}}'),
      );
    });

    it('after a refused cancel with no token, reports failure — never cancelled (#3068 G2.1)', async () => {
      // Mount sync sees the backend holding the flow; the refused cancel
      // then recovers the terminal provider ending via last-outcome (#3040 G3.1).
      vi.mocked(isOAuthFlowActive).mockResolvedValue(true);
      vi.mocked(takeOAuthFlowLastOutcome).mockResolvedValue({
        serverId: oauthServer.id,
        success: false,
        message: 'the authorization ended without storing a token',
      });
      renderOauthCard();

      expect(await screen.findByTestId('oauth-cancel')).toBeInTheDocument();

      vi.mocked(cancelOAuthFlow).mockRejectedValueOnce(
        new Error('no authorization in progress'),
      );
      fireEvent.click(screen.getByTestId('oauth-cancel'));

      expect(await screen.findByTestId('oauth-authorize')).toBeInTheDocument();
      const notice = await screen.findByTestId('oauth-result');
      expect(notice).toHaveTextContent(
        'Authorization failed: the authorization ended without storing a token',
      );
      expect(notice).not.toHaveTextContent('cancelled');
    });

    it('after a refused cancel that ended in success, shows the connected state (#3068 G2.1)', async () => {
      // The cancel is refused because the token is being stored; the flow
      // then ends with success and the last-outcome query lands Disconnect
      // only after hasOAuthToken reports true (#3040 G4.1).
      vi.mocked(isOAuthFlowActive).mockResolvedValue(true);
      vi.mocked(takeOAuthFlowLastOutcome).mockResolvedValue({
        serverId: oauthServer.id,
        success: true,
      });
      vi.mocked(hasOAuthToken)
        .mockResolvedValueOnce(false)
        .mockResolvedValue(true);
      renderOauthCard();

      expect(await screen.findByTestId('oauth-cancel')).toBeInTheDocument();

      vi.mocked(cancelOAuthFlow).mockRejectedValueOnce(
        new Error('OAuth flow is completing; token is being stored'),
      );
      fireEvent.click(screen.getByTestId('oauth-cancel'));

      expect(
        await screen.findByTestId('oauth-disconnect'),
      ).toBeInTheDocument();
      expect(screen.queryByTestId('oauth-result')).not.toBeInTheDocument();
    });

    it('shows the cancelled notice when a cancel succeeds on a remounted card (#3068 G2.1)', async () => {
      // No local store record: the card only knows the cancel succeeded
      // because the cancel RPC resolved — "cancelled" is then truthful.
      vi.mocked(isOAuthFlowActive).mockResolvedValueOnce(true);
      renderOauthCard();

      expect(await screen.findByTestId('oauth-cancel')).toBeInTheDocument();

      fireEvent.click(screen.getByTestId('oauth-cancel'));

      expect(await screen.findByTestId('oauth-authorize')).toBeInTheDocument();
      expect(await screen.findByTestId('oauth-result')).toHaveTextContent(
        'Authorization cancelled.',
      );
    });

    it('lets a card remounted onto a running flow settle into success (#3040 G2.2)', async () => {
      let resolveFlow: (value: string) => void = () => {};
      vi.mocked(startOAuthFlow).mockImplementationOnce(
        () =>
          new Promise<string>((resolve) => {
            resolveFlow = resolve;
          }),
      );

      const first = renderOauthCard();
      fireEvent.click(screen.getByTestId('oauth-authorize'));
      expect(await screen.findByTestId('oauth-cancel')).toBeInTheDocument();

      // Navigate away while the flow is still running.
      first.unmount();

      // A fresh card shows Cancel (flow from the store, not from its own state)
      // and learns about success without any user action.
      renderOauthCard();
      expect(await screen.findByTestId('oauth-cancel')).toBeInTheDocument();
      expect(screen.queryByTestId('oauth-authorize')).not.toBeInTheDocument();

      vi.mocked(hasOAuthToken).mockResolvedValue(true);
      await act(async () => resolveFlow('authorized'));

      expect(await screen.findByTestId('oauth-disconnect')).toBeInTheDocument();
      expect(screen.queryByTestId('oauth-cancel')).not.toBeInTheDocument();
      const toast = await import('sonner');
      expect(toast.toast.success).toHaveBeenCalledWith(
        'Successfully authenticated!',
      );
    });

    it('lets a card remounted onto a running flow recover after failure (#3040 G2.2)', async () => {
      let rejectFlow: (reason: Error) => void = () => {};
      vi.mocked(startOAuthFlow).mockImplementationOnce(
        () =>
          new Promise<string>((_resolve, reject) => {
            rejectFlow = reject;
          }),
      );

      const first = renderOauthCard();
      fireEvent.click(screen.getByTestId('oauth-authorize'));
      expect(await screen.findByTestId('oauth-cancel')).toBeInTheDocument();
      first.unmount();

      renderOauthCard();
      expect(await screen.findByTestId('oauth-cancel')).toBeInTheDocument();

      await act(async () =>
        rejectFlow(new Error('AUTH_TIMEOUT: OAuth authorization timed out')),
      );

      // Authorize is available again and the timed-out cause is on the card.
      const authorize = await screen.findByTestId('oauth-authorize');
      expect(authorize).toBeInTheDocument();
      expect(screen.queryByTestId('oauth-cancel')).not.toBeInTheDocument();
      expect(await screen.findByTestId('oauth-result')).toHaveTextContent(
        'timed out',
      );
    });

    it('shows Cancel when the backend already holds the flow (#3040 G1.5)', async () => {
      const { isOAuthFlowActive } = await import('@/lib/backend');
      vi.mocked(isOAuthFlowActive).mockResolvedValueOnce(true);

      renderCard(
        <ServerCard
          server={oauthServer}
          onEdit={noop}
          onDelete={noop}
          onToggleActive={noop}
        />,
      );

      expect(await screen.findByTestId('oauth-cancel')).toBeInTheDocument();
      expect(screen.queryByTestId('oauth-authorize')).not.toBeInTheDocument();
    });

    it('reports a timed-out flow as timed out, not cancelled (#3040 G1.6)', async () => {
      const { startOAuthFlow } = await import('@/lib/backend');
      vi.mocked(startOAuthFlow).mockRejectedValueOnce(
        new Error('AUTH_TIMEOUT: OAuth authorization timed out after 5 minutes'),
      );

      renderCard(
        <ServerCard
          server={oauthServer}
          onEdit={noop}
          onDelete={noop}
          onToggleActive={noop}
        />,
      );

      fireEvent.click(screen.getByTestId('oauth-authorize'));
      const notice = await screen.findByTestId('oauth-result');
      expect(notice).toHaveTextContent('timed out');
      expect(notice).not.toHaveTextContent('cancelled');
    });

    it('keeps a provider denial a provider error even when its text says cancelled (#3040 G1.6)', async () => {
      const { startOAuthFlow } = await import('@/lib/backend');
      vi.mocked(startOAuthFlow).mockRejectedValueOnce(
        new Error(
          'OAuth provider error (access_denied): The user cancelled the request',
        ),
      );

      renderCard(
        <ServerCard
          server={oauthServer}
          onEdit={noop}
          onDelete={noop}
          onToggleActive={noop}
        />,
      );

      fireEvent.click(screen.getByTestId('oauth-authorize'));
      const notice = await screen.findByTestId('oauth-result');
      expect(notice).toHaveTextContent(
        'OAuth provider error (access_denied): The user cancelled the request',
      );
    });

    it('applies a backend timeout ending via mcp-oauth-flow-ended (#3040 G3.1 / G4.3)', async () => {
      vi.mocked(isOAuthFlowActive).mockResolvedValue(true);
      renderOauthCard();
      expect(await screen.findByTestId('oauth-cancel')).toBeInTheDocument();
      expect(listen).toHaveBeenCalledWith(
        'mcp-oauth-flow-ended',
        expect.any(Function),
      );

      await emitFlowEnded({
        serverId: oauthServer.id,
        success: false,
        message: 'AUTH_TIMEOUT: OAuth authorization timed out after 5 minutes',
      });

      expect(await screen.findByTestId('oauth-authorize')).toBeInTheDocument();
      expect(await screen.findByTestId('oauth-result')).toHaveTextContent(
        'timed out',
      );
      expect(screen.queryByTestId('oauth-cancel')).not.toBeInTheDocument();
    });

    it('applies a backend provider-error ending via mcp-oauth-flow-ended (#3040 G4.3)', async () => {
      vi.mocked(isOAuthFlowActive).mockResolvedValue(true);
      renderOauthCard();
      expect(await screen.findByTestId('oauth-cancel')).toBeInTheDocument();

      await emitFlowEnded({
        serverId: oauthServer.id,
        success: false,
        message:
          'OAuth provider error (access_denied): The user cancelled the request',
      });

      const notice = await screen.findByTestId('oauth-result');
      expect(notice).toHaveTextContent(
        'OAuth provider error (access_denied): The user cancelled the request',
      );
      expect(notice).not.toHaveTextContent('Authorization cancelled');
    });

    it('applies a backend success ending via mcp-oauth-flow-ended and revalidates (#3040 G4.2 / G4.3)', async () => {
      const onRevalidate = vi.fn();
      vi.mocked(isOAuthFlowActive).mockResolvedValue(true);
      vi.mocked(hasOAuthToken)
        .mockResolvedValueOnce(false)
        .mockResolvedValue(true);
      renderCard(
        <ServerCard
          server={oauthServer}
          onEdit={noop}
          onDelete={noop}
          onToggleActive={noop}
          onRevalidate={onRevalidate}
        />,
      );
      expect(await screen.findByTestId('oauth-cancel')).toBeInTheDocument();

      await emitFlowEnded({
        serverId: oauthServer.id,
        success: true,
      });

      expect(
        await screen.findByTestId('oauth-disconnect'),
      ).toBeInTheDocument();
      expect(screen.queryByTestId('oauth-cancel')).not.toBeInTheDocument();
      expect(screen.queryByTestId('oauth-result')).not.toBeInTheDocument();
      await vi.waitFor(() =>
        expect(safeInvoke).toHaveBeenCalledWith('probe_mcp_server', {
          serverId: oauthServer.id,
        }),
      );
      await vi.waitFor(() => expect(onRevalidate).toHaveBeenCalled());
      // Exactly one probe / revalidate per settle (#3068 G1.2).
      expect(
        vi
          .mocked(safeInvoke)
          .mock.calls.filter(([cmd]) => cmd === 'probe_mcp_server'),
      ).toHaveLength(1);
      expect(onRevalidate).toHaveBeenCalledTimes(1);
    });

    it('probes exactly once per settle on the card that owns the flow (#3068 G1.2)', async () => {
      // The initiating card also settles through the subscription path only;
      // handleAuthorize must not probe a second time (#3068 G1.2).
      const onRevalidate = vi.fn();
      vi.mocked(startOAuthFlow).mockResolvedValue('authorized');
      vi.mocked(hasOAuthToken)
        .mockResolvedValueOnce(false)
        .mockResolvedValue(true);
      renderCard(
        <ServerCard
          server={oauthServer}
          onEdit={noop}
          onDelete={noop}
          onToggleActive={noop}
          onRevalidate={onRevalidate}
        />,
      );

      fireEvent.click(screen.getByTestId('oauth-authorize'));
      expect(
        await screen.findByTestId('oauth-disconnect'),
      ).toBeInTheDocument();
      const toast = await import('sonner');
      await vi.waitFor(() =>
        expect(toast.toast.success).toHaveBeenCalledWith(
          'Successfully authenticated!',
        ),
      );
      await vi.waitFor(() => expect(onRevalidate).toHaveBeenCalled());

      expect(
        vi
          .mocked(safeInvoke)
          .mock.calls.filter(([cmd]) => cmd === 'probe_mcp_server'),
      ).toHaveLength(1);
      expect(onRevalidate).toHaveBeenCalledTimes(1);
    });

    it('probes exactly once when the card unmounts before the flow settles (#3068 G2.1)', async () => {
      // The verification must not depend on a mounted card: the store runs
      // it on settle even when the initiating card is gone (#3068 G2.1).
      let resolveStart: (value: string) => void = () => {};
      vi.mocked(startOAuthFlow).mockImplementation(
        () =>
          new Promise<string>((resolve) => {
            resolveStart = resolve;
          }),
      );
      const rendered = renderOauthCard();

      fireEvent.click(screen.getByTestId('oauth-authorize'));
      expect(await screen.findByTestId('oauth-cancel')).toBeInTheDocument();

      rendered.unmount();
      await act(async () => {
        resolveStart('authorized');
      });

      await vi.waitFor(() =>
        expect(safeInvoke).toHaveBeenCalledWith('probe_mcp_server', {
          serverId: oauthServer.id,
        }),
      );
      expect(
        vi
          .mocked(safeInvoke)
          .mock.calls.filter(([cmd]) => cmd === 'probe_mcp_server'),
      ).toHaveLength(1);
    });

    it('revalidates even when the post-settle probe fails (#3068 G2.1)', async () => {
      // A probe failure is logged by the store, never swallowed silently,
      // and the card still revalidates (#3068 G2.1 review note).
      const onRevalidate = vi.fn();
      vi.mocked(startOAuthFlow).mockResolvedValue('authorized');
      vi.mocked(safeInvoke).mockRejectedValueOnce(new Error('probe down'));
      renderCard(
        <ServerCard
          server={oauthServer}
          onEdit={noop}
          onDelete={noop}
          onToggleActive={noop}
          onRevalidate={onRevalidate}
        />,
      );

      fireEvent.click(screen.getByTestId('oauth-authorize'));
      await vi.waitFor(() => expect(onRevalidate).toHaveBeenCalledTimes(1));
    });

    it('does not resurrect Authorized after Disconnect remount (#3040 G4.1)', async () => {
      vi.mocked(hasOAuthToken).mockResolvedValue(true);
      vi.mocked(takeOAuthFlowLastOutcome).mockResolvedValue({
        serverId: oauthServer.id,
        success: true,
      });
      const first = renderOauthCard();
      expect(await screen.findByTestId('oauth-disconnect')).toBeInTheDocument();

      fireEvent.click(screen.getByTestId('oauth-disconnect'));
      await vi.waitFor(() =>
        expect(revokeOAuthToken).toHaveBeenCalledWith(oauthServer.id),
      );
      first.unmount();

      // Stale last-outcome may still exist until revoke clears it; a cold
      // remount must trust hasOAuthToken only (#3040 G4.1).
      vi.mocked(hasOAuthToken).mockResolvedValue(false);
      vi.mocked(isOAuthFlowActive).mockResolvedValue(false);
      renderOauthCard();
      expect(await screen.findByTestId('oauth-authorize')).toBeInTheDocument();
      expect(screen.queryByTestId('oauth-disconnect')).not.toBeInTheDocument();
      expect(screen.queryByTestId('oauth-result')).not.toBeInTheDocument();
    });
  });
});
