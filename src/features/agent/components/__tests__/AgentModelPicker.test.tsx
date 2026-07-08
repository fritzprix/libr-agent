import { fireEvent, render, screen } from '@testing-library/react';
import '@testing-library/jest-dom';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { AgentModelPicker } from '../AgentModelPicker';
import { TooltipProvider } from '@/components/ui/tooltip';

const mockUseAgentModelsState = vi.hoisted(() => ({
  availableModels: {
    'model-1': {
      name: 'Model 1',
    },
  },
  isRefreshing: false,
  refreshModels: vi.fn().mockResolvedValue(undefined),
  canRefresh: true,
  refreshBlockedReason: 'allowed',
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: string | Record<string, unknown>) => {
      if (typeof options === 'string') {
        return options;
      }
      if (options && typeof options === 'object' && 'defaultValue' in options) {
        return String(options.defaultValue);
      }
      return key;
    },
  }),
}));

vi.mock('@/components/ui/select', () => ({
  Select: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  SelectContent: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  SelectItem: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  SelectTrigger: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  SelectValue: ({ placeholder }: { placeholder?: string }) => (
    <span>{placeholder}</span>
  ),
}));

vi.mock('../../hooks/useAgentModels', () => ({
  useAgentModels: () => mockUseAgentModelsState,
}));

describe('AgentModelPicker', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockUseAgentModelsState.availableModels = {
      'model-1': { name: 'Model 1' },
    };
    mockUseAgentModelsState.isRefreshing = false;
    mockUseAgentModelsState.canRefresh = true;
    mockUseAgentModelsState.refreshBlockedReason = 'allowed';
  });

  it('revalidates models when refresh is available', () => {
    render(
      <TooltipProvider>
        <AgentModelPicker currentModel="model-1" currentProvider="ollama" />
      </TooltipProvider>
    );

    const refreshButton = screen.getByRole('button', {
      name: 'agent.modelPicker.refreshModels',
    });

    fireEvent.click(refreshButton);

    expect(mockUseAgentModelsState.refreshModels).toHaveBeenCalledTimes(1);
    expect(refreshButton).toBeEnabled();
  });

  it('keeps the refresh button visible but disabled when an API key is required', () => {
    mockUseAgentModelsState.canRefresh = false;
    mockUseAgentModelsState.refreshBlockedReason = 'missing-api-key';

    render(
      <TooltipProvider>
        <AgentModelPicker currentModel="model-1" currentProvider="anthropic" />
      </TooltipProvider>
    );

    const refreshButton = screen.getByRole('button', {
      name: 'Add an API key to enable model refresh',
    });

    expect(refreshButton).toBeDisabled();
  });

  it('hides refresh when custom openai model discovery is intentionally disabled', () => {
    mockUseAgentModelsState.canRefresh = false;
    mockUseAgentModelsState.refreshBlockedReason = 'custom-openai-model';

    render(
      <TooltipProvider>
        <AgentModelPicker currentModel="model-1" currentProvider="openai" />
      </TooltipProvider>
    );

    expect(
      screen.queryByRole('button', {
        name: /refresh/i,
      }),
    ).not.toBeInTheDocument();
  });
});
