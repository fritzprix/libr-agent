import { fireEvent, render, screen } from '@testing-library/react';
import '@testing-library/jest-dom';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { AgentPanelsProvider, useAgentPanels } from '@/context/AgentPanelsContext';
import { AgentSidePanelShell } from '../AgentSidePanelShell';

vi.mock('../AgentWorkspacePanel', () => ({
  AgentWorkspacePanel: ({ isVisible }: { isVisible?: boolean }) => (
    <div data-testid="workspace-tab" data-visible={String(Boolean(isVisible))}>
      workspace
    </div>
  ),
}));

vi.mock('../AgentProcessPanel', () => ({
  AgentProcessPanel: ({ isVisible }: { isVisible?: boolean }) => (
    <div data-testid="processes-tab" data-visible={String(Boolean(isVisible))}>
      processes
    </div>
  ),
}));

vi.mock('../AgentPlanningPanel', () => ({
  AgentPlanningPanel: ({ isVisible }: { isVisible?: boolean }) => (
    <div data-testid="planning-tab" data-visible={String(Boolean(isVisible))}>
      planning
    </div>
  ),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, defaultOrOptions?: string | Record<string, unknown>) => {
      if (typeof defaultOrOptions === 'string') {
        return defaultOrOptions;
      }
      if (
        defaultOrOptions &&
        typeof defaultOrOptions === 'object' &&
        'defaultValue' in defaultOrOptions &&
        typeof defaultOrOptions.defaultValue === 'string'
      ) {
        return defaultOrOptions.defaultValue.replace(
          '{{tab}}',
          String(defaultOrOptions.tab ?? ''),
        );
      }
      return key;
    },
  }),
}));

function OpenShell({
  children,
  tab = 'workspace',
}: {
  children: ReactNode;
  tab?: 'workspace' | 'processes' | 'planning';
}) {
  const { openPanel } = useAgentPanels();
  return (
    <div>
      <button type="button" onClick={() => openPanel(tab)}>
        open
      </button>
      {children}
    </div>
  );
}

function renderShell(tab: 'workspace' | 'processes' | 'planning' = 'workspace') {
  return render(
    <AgentPanelsProvider>
      <OpenShell tab={tab}>
        <AgentSidePanelShell isVisible />
      </OpenShell>
    </AgentPanelsProvider>,
  );
}

describe('AgentSidePanelShell', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders three tab triggers and keeps inactive panels mounted', () => {
    renderShell('workspace');
    fireEvent.click(screen.getByRole('button', { name: 'open' }));

    expect(
      screen.getByRole('tab', { name: 'Workspace' }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole('tab', { name: 'Processes' }),
    ).toBeInTheDocument();
    expect(screen.getByRole('tab', { name: 'Planning' })).toBeInTheDocument();

    expect(screen.getByTestId('workspace-tab')).toHaveAttribute(
      'data-visible',
      'true',
    );
    expect(screen.getByTestId('processes-tab')).toHaveAttribute(
      'data-visible',
      'false',
    );
    expect(screen.getByTestId('planning-tab')).toHaveAttribute(
      'data-visible',
      'false',
    );
  });

  it('switches active tab content when a trigger is clicked', () => {
    function Switcher({ children }: { children: ReactNode }) {
      const { openPanel, setActiveTab } = useAgentPanels();
      return (
        <div>
          <button type="button" onClick={() => openPanel('workspace')}>
            open
          </button>
          <button type="button" onClick={() => setActiveTab('processes')}>
            go-processes
          </button>
          {children}
        </div>
      );
    }

    render(
      <AgentPanelsProvider>
        <Switcher>
          <AgentSidePanelShell isVisible />
        </Switcher>
      </AgentPanelsProvider>,
    );

    fireEvent.click(screen.getByRole('button', { name: 'open' }));
    fireEvent.click(screen.getByRole('button', { name: 'go-processes' }));

    expect(screen.getByTestId('processes-tab')).toHaveAttribute(
      'data-visible',
      'true',
    );
    expect(screen.getByTestId('workspace-tab')).toHaveAttribute(
      'data-visible',
      'false',
    );
  });

  it('shows an attention marker on a tab with updates', () => {
    function MarkAttention({ children }: { children: ReactNode }) {
      const { markPanelAttention, openPanel } = useAgentPanels();
      return (
        <div>
          <button
            type="button"
            onClick={() => {
              openPanel('workspace');
              markPanelAttention('processes');
            }}
          >
            seed
          </button>
          {children}
        </div>
      );
    }

    render(
      <AgentPanelsProvider>
        <MarkAttention>
          <AgentSidePanelShell isVisible />
        </MarkAttention>
      </AgentPanelsProvider>,
    );

    fireEvent.click(screen.getByRole('button', { name: 'seed' }));

    expect(
      screen.getByRole('tab', { name: 'Processes (has updates)' }),
    ).toBeInTheDocument();
    expect(screen.getByTestId('panel-attention-dot')).toBeInTheDocument();
  });

  it('closes the shell from the explicit close control', () => {
    function ShellState({ children }: { children: ReactNode }) {
      const { openPanel, isShellOpen } = useAgentPanels();
      return (
        <div>
          <button type="button" onClick={() => openPanel('workspace')}>
            open
          </button>
          <span data-testid="shell-open">{String(isShellOpen())}</span>
          {children}
        </div>
      );
    }

    render(
      <AgentPanelsProvider>
        <ShellState>
          <AgentSidePanelShell isVisible />
        </ShellState>
      </AgentPanelsProvider>,
    );

    fireEvent.click(screen.getByRole('button', { name: 'open' }));
    expect(screen.getByTestId('shell-open')).toHaveTextContent('true');

    fireEvent.click(
      screen.getByRole('button', { name: 'Close agent panels' }),
    );
    expect(screen.getByTestId('shell-open')).toHaveTextContent('false');
  });
});
