import { fireEvent, render, screen } from '@testing-library/react';
import '@testing-library/jest-dom';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { AgentModelPicker } from '../AgentModelPicker';

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
  Select: ({
    children,
    value,
    onValueChange,
  }: {
    children: ReactNode;
    value?: string;
    onValueChange?: (value: string) => void;
  }) => (
    <div data-testid="select" data-value={value ?? ''}>
      <button
        type="button"
        data-testid="select-trigger"
        onClick={() => undefined}
      >
        {value}
      </button>
      <div
        data-testid="select-options"
        data-on-change={onValueChange ? 'yes' : 'no'}
      >
        {children}
      </div>
      {onValueChange ? (
        <button
          type="button"
          data-testid={`select-set-${value || 'empty'}`}
          onClick={() => onValueChange('custom:local1')}
        >
          pick-custom
        </button>
      ) : null}
      {onValueChange ? (
        <button
          type="button"
          data-testid="select-emit-empty"
          onClick={() => onValueChange('')}
        >
          emit-empty
        </button>
      ) : null}
    </div>
  ),
  SelectContent: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  SelectItem: ({
    children,
    value,
  }: {
    children: ReactNode;
    value: string;
  }) => <div data-value={value}>{children}</div>,
  SelectTrigger: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  SelectValue: ({ placeholder }: { placeholder?: string }) => (
    <span>{placeholder}</span>
  ),
}));

vi.mock('../../hooks/useAgentModels', () => ({
  useAgentModels: () => mockUseAgentModelsState,
}));

vi.mock('@/hooks/use-settings', () => ({
  useSettings: () => ({
    value: {
      customProviders: [
        {
          id: 'local1',
          name: 'Local vLLM',
          baseUrl: 'http://127.0.0.1:8000/v1',
        },
      ],
      serviceConfigs: {},
    },
  }),
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
      <AgentModelPicker currentModel="model-1" currentProvider="ollama" />,
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
      <AgentModelPicker currentModel="model-1" currentProvider="anthropic" />,
    );

    const refreshButtonWrapper = screen.getByRole('button', {
      name: 'Add an API key to enable model refresh',
    });

    expect(refreshButtonWrapper).toHaveAttribute('aria-disabled', 'true');

    const innerButton = refreshButtonWrapper.querySelector('button');
    expect(innerButton).toBeDisabled();
    expect(innerButton).not.toHaveAttribute('title');
  });

  it('hides refresh when custom openai model discovery is intentionally disabled', () => {
    mockUseAgentModelsState.canRefresh = false;
    mockUseAgentModelsState.refreshBlockedReason = 'custom-openai-model';

    render(
      <AgentModelPicker currentModel="model-1" currentProvider="openai" />,
    );

    expect(
      screen.queryByRole('button', {
        name: /refresh/i,
      }),
    ).not.toBeInTheDocument();
  });

  it('clears foreign model ids when switching to a custom provider', () => {
    const onConfigUpdate = vi.fn();

    render(
      <AgentModelPicker
        currentModel="gpt-4o"
        currentProvider="openai"
        onConfigUpdate={onConfigUpdate}
      />,
    );

    // First Select is the provider picker
    const pickCustomButtons = screen.getAllByText('pick-custom');
    fireEvent.click(pickCustomButtons[0]);

    expect(onConfigUpdate).toHaveBeenCalledWith('', 'custom:local1');

    onConfigUpdate.mockClear();
    fireEvent.click(screen.getAllByTestId('select-emit-empty')[0]);
    expect(onConfigUpdate).not.toHaveBeenCalled();
  });
});
