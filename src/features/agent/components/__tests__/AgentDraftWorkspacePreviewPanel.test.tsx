import React from 'react';
import { fireEvent, render, screen } from '@testing-library/react';
import '@testing-library/jest-dom/vitest';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { AgentDraftWorkspacePreviewPanel } from '../AgentDraftWorkspacePreviewPanel';

const mockUseDraftWorkspacePreviewTree = vi.fn();

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, fallback?: string) => fallback ?? key,
  }),
}));

vi.mock('@/components/ui/tooltip', () => ({
  Tooltip: ({ children }: { children?: React.ReactNode }) => <div>{children}</div>,
  TooltipTrigger: ({ children, asChild }: { children?: React.ReactNode; asChild?: boolean }) => <div>{children}</div>,
  TooltipContent: ({ children }: { children?: React.ReactNode }) => <div>{children}</div>,
}));

vi.mock('../workspace-panel/useDraftWorkspacePreviewTree', () => ({
  useDraftWorkspacePreviewTree: (...args: unknown[]) =>
    mockUseDraftWorkspacePreviewTree(...args),
}));

describe('AgentDraftWorkspacePreviewPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks();

    mockUseDraftWorkspacePreviewTree.mockReturnValue({
      fileTree: [
        {
          id: 'src',
          name: 'src',
          path: 'src',
          isDirectory: true,
          isExpanded: true,
          children: [
            {
              id: 'src/main.ts',
              name: 'main.ts',
              path: 'src/main.ts',
              isDirectory: false,
            },
          ],
        },
      ],
      loading: false,
      error: null,
      refresh: vi.fn(),
      toggleDirectory: vi.fn(),
    });
  });

  it('renders read-only workspace preview details and tree items', () => {
    const workspacePath = 'C:\\workspace';

    render(
      <AgentDraftWorkspacePreviewPanel
        workspacePath={workspacePath}
        onClear={vi.fn()}
      />,
    );

    expect(
      mockUseDraftWorkspacePreviewTree,
    ).toHaveBeenCalledWith(workspacePath);
    expect(screen.getByText('Workspace Override Active')).toBeInTheDocument();
    expect(
      screen.getByText('Read-only preview before session start'),
    ).toBeInTheDocument();
    expect(screen.getByText(workspacePath)).toBeInTheDocument();
    expect(screen.getByText('src')).toBeInTheDocument();
    expect(screen.getByText('main.ts')).toBeInTheDocument();
  });

  it('wires refresh and clear actions', () => {
    const workspacePath = 'C:\\workspace';
    const refresh = vi.fn();
    const onClear = vi.fn();

    mockUseDraftWorkspacePreviewTree.mockReturnValue({
      fileTree: [],
      loading: false,
      error: null,
      refresh,
      toggleDirectory: vi.fn(),
    });

    render(
      <AgentDraftWorkspacePreviewPanel
        workspacePath={workspacePath}
        onClear={onClear}
      />,
    );

    fireEvent.click(screen.getByLabelText('agent.workspace.refreshAria'));
    fireEvent.click(screen.getByLabelText('Close'));

    expect(refresh).toHaveBeenCalledTimes(1);
    expect(onClear).toHaveBeenCalledTimes(1);
    expect(screen.getByText('agent.workspace.refreshAria')).toBeInTheDocument();
    expect(screen.getByText('Close')).toBeInTheDocument();
  });
});
