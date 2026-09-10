import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom';

import { DockerErrorModal } from '../DockerErrorModal';

const mocks = vi.hoisted(() => ({
  waitForDockerReady: vi.fn(),
  isDockerDesktopLaunchSupported: vi.fn(),
  startDockerDesktop: vi.fn(),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, fallback?: string) => fallback ?? key,
  }),
}));

vi.mock('@/lib/backend/dockerHealth', () => ({
  waitForDockerReady: (...args: unknown[]) =>
    mocks.waitForDockerReady(...args),
}));

vi.mock('@/lib/backend/workspace', () => ({
  isDockerDesktopLaunchSupported: () =>
    mocks.isDockerDesktopLaunchSupported(),
  startDockerDesktop: () => mocks.startDockerDesktop(),
}));

describe('DockerErrorModal', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.isDockerDesktopLaunchSupported.mockResolvedValue(false);
    mocks.waitForDockerReady.mockResolvedValue(true);
  });

  it('shows not-installed copy and a host-mode action', () => {
    const onRunInHostMode = vi.fn();
    const onClose = vi.fn();

    render(
      <DockerErrorModal
        isOpen
        onClose={onClose}
        onRetry={vi.fn()}
        onRunInHostMode={onRunInHostMode}
        errorDetails="DOCKER CLI missing"
        notInstalled
      />,
    );

    expect(
      screen.getByText('agent.draft.dockerNotInstalledTitle'),
    ).toBeInTheDocument();
    expect(
      screen.getByText('agent.draft.dockerNotInstalledDescription'),
    ).toBeInTheDocument();
    expect(
      screen.getByRole('link', { name: 'agent.draft.dockerInstallLink' }),
    ).toHaveAttribute(
      'href',
      'https://www.docker.com/products/docker-desktop/',
    );

    fireEvent.click(
      screen.getByRole('button', { name: 'agent.draft.dockerRunInHostMode' }),
    );

    expect(onClose).toHaveBeenCalledTimes(1);
    expect(onRunInHostMode).toHaveBeenCalledTimes(1);
  });

  it('keeps daemon-offline copy when Docker is installed but unavailable', () => {
    render(
      <DockerErrorModal
        isOpen
        onClose={vi.fn()}
        onRetry={vi.fn()}
        onRunInHostMode={vi.fn()}
        notInstalled={false}
      />,
    );

    expect(
      screen.getByText('agent.draft.dockerErrorTitle'),
    ).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: 'agent.draft.dockerRunInHostMode' }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole('link', { name: 'agent.draft.dockerInstallLink' }),
    ).not.toBeInTheDocument();
  });
});
