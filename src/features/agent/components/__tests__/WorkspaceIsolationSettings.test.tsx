import { render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom';

import { WorkspaceIsolationSettings } from '../WorkspaceIsolationSettings';
import { DOCKER_NOT_INSTALLED_PREFIX } from '@/lib/backend/errors';

const mocks = vi.hoisted(() => ({
  checkDockerHealth: vi.fn(),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, fallback?: string) => fallback ?? key,
  }),
}));

vi.mock('@/lib/backend/dockerHealth', () => ({
  checkDockerHealth: (...args: unknown[]) => mocks.checkDockerHealth(...args),
}));

vi.mock('@/components/ui/tooltip', () => ({
  Tooltip: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
  TooltipTrigger: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
  TooltipContent: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
}));

describe('WorkspaceIsolationSettings', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('does not probe Docker health in host mode', () => {
    render(
      <WorkspaceIsolationSettings
        switchId="docker-isolation"
        workspaceIsolation="host"
        setWorkspaceIsolation={vi.fn()}
        dockerImage="python:3.11-slim"
        setDockerImage={vi.fn()}
      />,
    );

    expect(mocks.checkDockerHealth).not.toHaveBeenCalled();
    expect(
      screen.queryByText('Docker is not installed or is not available on PATH.'),
    ).not.toBeInTheDocument();
  });

  it('shows an inline warning when Docker is not installed', async () => {
    mocks.checkDockerHealth.mockRejectedValueOnce(
      new Error(
        `${DOCKER_NOT_INSTALLED_PREFIX} Docker CLI is not installed or not available on PATH`,
      ),
    );

    render(
      <WorkspaceIsolationSettings
        switchId="docker-isolation"
        workspaceIsolation="docker"
        setWorkspaceIsolation={vi.fn()}
        dockerImage="python:3.11-slim"
        setDockerImage={vi.fn()}
      />,
    );

    expect(
      await screen.findByText(
        'Docker is not installed or is not available on PATH.',
      ),
    ).toBeInTheDocument();
    expect(
      screen.getByText(
        'Install Docker Desktop, start the engine, or switch back to host mode.',
      ),
    ).toBeInTheDocument();
  });

  it('shows a daemon-offline warning when healthcheck fails without the installed prefix', async () => {
    mocks.checkDockerHealth.mockRejectedValueOnce(
      new Error('DOCKER_NOT_AVAILABLE: Docker is not available'),
    );

    render(
      <WorkspaceIsolationSettings
        switchId="docker-isolation"
        workspaceIsolation="docker"
        setWorkspaceIsolation={vi.fn()}
        dockerImage="python:3.11-slim"
        setDockerImage={vi.fn()}
      />,
    );

    expect(
      await screen.findByText('Docker daemon is not running.'),
    ).toBeInTheDocument();
  });
});
