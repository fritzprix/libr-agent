import { fireEvent, render, screen } from '@testing-library/react';
import '@testing-library/jest-dom';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { encodeModelChoice } from '@/lib/ai-service/model-choice-encoding';
import type { ModelInfo } from '@/lib/llm-config-manager';
import { AgentModelPicker } from '../AgentModelPicker';

const mockGroupedModelsState = vi.hoisted(() => ({
  groupedModels: [
    {
      providerId: 'ollama',
      label: 'Ollama',
      models: {
        'model-1': {
          id: 'model-1',
          name: 'Model 1',
          contextWindow: 128000,
          supportReasoning: true,
          supportTools: true,
          supportStreaming: true,
          cost: { input: 0, output: 0 },
          description: 'Test model',
        },
      },
    },
    {
      providerId: 'custom:local1',
      label: 'Local vLLM',
      models: {},
    },
  ],
  hasConfiguredProviders: true,
  isRefreshing: false,
  refreshModels: vi.fn().mockResolvedValue(undefined),
  canRefresh: true,
  refreshBlockedReason: 'allowed',
  getModelInfo: vi.fn((): ModelInfo | undefined => undefined),
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
    <div data-testid="model-select" data-value={value ?? ''}>
      <button
        type="button"
        data-testid="model-select-trigger"
        onClick={() => undefined}
      >
        {value}
      </button>
      <div data-testid="model-select-options">
        {children}
      </div>
      {onValueChange ? (
        <button
          type="button"
          data-testid="model-select-pick-custom"
          onClick={() =>
            onValueChange(encodeModelChoice('custom:local1', 'local-model'))
          }
        >
          pick-custom
        </button>
      ) : null}
      {onValueChange ? (
        <button
          type="button"
          data-testid="model-select-emit-empty"
          onClick={() => onValueChange('')}
        >
          emit-empty
        </button>
      ) : null}
    </div>
  ),
  SelectContent: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  SelectGroup: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  SelectItem: ({
    children,
    value,
  }: {
    children: ReactNode;
    value: string;
  }) => <div data-value={value}>{children}</div>,
  SelectLabel: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  SelectTrigger: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  SelectValue: ({ placeholder }: { placeholder?: string }) => (
    <span>{placeholder}</span>
  ),
}));

vi.mock('@/features/settings/components/ThinkingEffortControl', () => ({
  ThinkingEffortControl: ({
    onThinkingEffortChange,
    disabled,
  }: {
    onThinkingEffortChange: (effort: string) => void;
    disabled?: boolean;
  }) => (
    <button
      type="button"
      data-testid="thinking-effort-control"
      disabled={disabled}
      onClick={() => onThinkingEffortChange('high')}
    >
      effort
    </button>
  ),
}));

vi.mock('../../hooks/useGroupedAgentModels', () => ({
  useGroupedAgentModels: () => mockGroupedModelsState,
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
      advanced: {
        thinkingEffort: 'medium',
      },
    },
  }),
}));

describe('AgentModelPicker', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockGroupedModelsState.isRefreshing = false;
    mockGroupedModelsState.canRefresh = true;
    mockGroupedModelsState.refreshBlockedReason = 'allowed';
    mockGroupedModelsState.getModelInfo.mockReset();
    mockGroupedModelsState.getModelInfo.mockReturnValue(undefined);
  });

  it('revalidates models when refresh is available', () => {
    render(
      <AgentModelPicker currentModel="model-1" currentProvider="ollama" />,
    );

    const refreshButton = screen.getByRole('button', {
      name: 'agent.modelPicker.refreshModels',
    });

    fireEvent.click(refreshButton);

    expect(mockGroupedModelsState.refreshModels).toHaveBeenCalledTimes(1);
    expect(refreshButton).toBeEnabled();
  });

  it('keeps the refresh button visible but disabled when an API key is required', () => {
    mockGroupedModelsState.canRefresh = false;
    mockGroupedModelsState.refreshBlockedReason = 'missing-api-key';

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
    mockGroupedModelsState.canRefresh = false;
    mockGroupedModelsState.refreshBlockedReason = 'custom-openai-model';

    render(
      <AgentModelPicker currentModel="model-1" currentProvider="openai" />,
    );

    expect(
      screen.queryByRole('button', {
        name: /refresh/i,
      }),
    ).not.toBeInTheDocument();
  });

  it('updates provider and model atomically from grouped selection', () => {
    const onConfigUpdate = vi.fn();

    render(
      <AgentModelPicker
        currentModel="gpt-4o"
        currentProvider="openai"
        onConfigUpdate={onConfigUpdate}
      />,
    );

    fireEvent.click(screen.getByTestId('model-select-pick-custom'));

    expect(onConfigUpdate).toHaveBeenCalledWith('local-model', 'custom:local1');

    onConfigUpdate.mockClear();
    fireEvent.click(screen.getByTestId('model-select-emit-empty'));
    expect(onConfigUpdate).not.toHaveBeenCalled();
  });

  it('renders thinking effort control when enabled', () => {
    const onThinkingEffortChange = vi.fn();

    render(
      <AgentModelPicker
        currentModel="model-1"
        currentProvider="ollama"
        showThinkingEffort
        onThinkingEffortChange={onThinkingEffortChange}
      />,
    );

    fireEvent.click(screen.getByTestId('thinking-effort-control'));
    expect(onThinkingEffortChange).toHaveBeenCalledWith('high');
  });

  it('keeps thinking effort enabled for models without supportReasoning metadata', () => {
    mockGroupedModelsState.getModelInfo.mockReturnValue({
      id: 'gpt-4o',
      name: 'GPT-4o',
      contextWindow: 128000,
      supportReasoning: false,
      supportTools: true,
      supportStreaming: true,
      cost: { input: 0, output: 0 },
      description: 'Non-reasoning OpenAI model',
    });

    const onThinkingEffortChange = vi.fn();

    render(
      <AgentModelPicker
        currentModel="gpt-4o"
        currentProvider="openai"
        showThinkingEffort
        onThinkingEffortChange={onThinkingEffortChange}
      />,
    );

    const control = screen.getByTestId('thinking-effort-control');
    expect(control).not.toBeDisabled();
    fireEvent.click(control);
    expect(onThinkingEffortChange).toHaveBeenCalledWith('high');
  });
});
