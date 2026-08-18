import { fireEvent, render, screen } from '@testing-library/react';
import '@testing-library/jest-dom';
import type { ChangeEvent, ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { AIServiceProvider } from '@/lib/ai-service';
import type { ServiceConfig } from '@/context/SettingsContext';
import AIModelsTab from '../AIModelsTab';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, defaultValue?: string) => defaultValue || key,
  }),
}));

const mockAgentModelPicker = vi.fn((props: unknown) => {
  void props;
  return <div data-testid="model-picker" />;
});

vi.mock('@/features/agent/components/AgentModelPicker', () => ({
  AgentModelPicker: (props: unknown) => mockAgentModelPicker(props),
}));

vi.mock('@/components/ui', () => ({
  Button: ({
    children,
    onClick,
  }: {
    children: ReactNode;
    onClick?: () => void;
  }) => <button onClick={onClick}>{children}</button>,
  Input: ({
    value,
    onChange,
    placeholder,
    type,
  }: {
    value?: string;
    onChange?: (event: ChangeEvent<HTMLInputElement>) => void;
    placeholder?: string;
    type?: string;
  }) => (
    <input
      type={type}
      value={value}
      placeholder={placeholder}
      onChange={onChange}
    />
  ),
  Textarea: ({
    value,
    onChange,
    placeholder,
  }: {
    value?: string;
    onChange?: (event: ChangeEvent<HTMLTextAreaElement>) => void;
    placeholder?: string;
  }) => (
    <textarea value={value} placeholder={placeholder} onChange={onChange} />
  ),
  Slider: () => <div data-testid="mock-slider" />,
  Card: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  CardHeader: ({ children }: { children: ReactNode }) => (
    <div>{children}</div>
  ),
  CardTitle: ({ children }: { children: ReactNode }) => (
    <div>{children}</div>
  ),
  CardContent: ({ children }: { children: ReactNode }) => (
    <div>{children}</div>
  ),
  Tooltip: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  TooltipTrigger: ({ children }: { children: ReactNode }) => (
    <div>{children}</div>
  ),
  TooltipContent: ({ children }: { children: ReactNode }) => (
    <div>{children}</div>
  ),
  Checkbox: ({
    checked,
    onCheckedChange,
    id,
  }: {
    checked?: boolean;
    onCheckedChange?: (checked: boolean) => void;
    id?: string;
  }) => (
    <input
      id={id}
      type="checkbox"
      checked={checked}
      onChange={(event) => onCheckedChange?.(event.target.checked)}
    />
  ),
}));

const temperatureProps = {
  temperatureOverrideEnabled: false,
  temperature: 0.7,
  onTemperatureOverrideEnabledChange: vi.fn(),
  onTemperatureChange: vi.fn(),
};

describe('AIModelsTab', () => {
  beforeEach(() => {
    mockAgentModelPicker.mockClear();
  });

  it('does not pass draft service config into the model picker', () => {
    const serviceConfigs: Record<AIServiceProvider, ServiceConfig> = {
      [AIServiceProvider.Groq]: {},
      [AIServiceProvider.OpenAI]: {
        apiKey: 'draft-key',
        baseUrl: 'https://draft.example.com/v1',
        use3rdParty: true,
        customModelId: 'draft-model',
      },
      [AIServiceProvider.Anthropic]: {},
      [AIServiceProvider.Gemini]: {},
      [AIServiceProvider.Fireworks]: {},
      [AIServiceProvider.Cerebras]: {},
      [AIServiceProvider.Ollama]: {},
      [AIServiceProvider.OpenRouter]: {},
      [AIServiceProvider.Empty]: {},
    };

    render(
      <AIModelsTab
        serviceConfigs={serviceConfigs}
        customProviders={[]}
        providerEntries={[AIServiceProvider.OpenAI]}
        localPreferredModel={{
          provider: AIServiceProvider.OpenAI,
          model: 'gpt-4o',
        }}
        localFallbackModel={undefined}
        onPendingChange={vi.fn()}
        onCustomProviderChange={vi.fn()}
        onAddCustomProvider={vi.fn()}
        onRemoveCustomProvider={vi.fn()}
        onPreferredModelChange={vi.fn()}
        onFallbackModelChange={vi.fn()}
        {...temperatureProps}
      />,
    );

    expect(mockAgentModelPicker).toHaveBeenCalledWith(
      expect.not.objectContaining({
        serviceConfigOverride: expect.anything(),
      }),
    );
    expect(mockAgentModelPicker).toHaveBeenCalledWith(
      expect.objectContaining({
        currentModel: 'gpt-4o',
        currentProvider: AIServiceProvider.OpenAI,
      }),
    );
  });

  it('rerenders provider cards when serviceConfigs change', () => {
    const onPendingChange = vi.fn();
    const serviceConfigs: Record<AIServiceProvider, ServiceConfig> = {
      [AIServiceProvider.Groq]: {},
      [AIServiceProvider.OpenAI]: {},
      [AIServiceProvider.Anthropic]: {},
      [AIServiceProvider.Gemini]: {},
      [AIServiceProvider.Fireworks]: {},
      [AIServiceProvider.Cerebras]: {},
      [AIServiceProvider.Ollama]: {
        baseUrl: 'http://127.0.0.1:11434',
      },
      [AIServiceProvider.OpenRouter]: {},
      [AIServiceProvider.Empty]: {},
    };

    const baseProps = {
      customProviders: [],
      providerEntries: [AIServiceProvider.Ollama],
      localPreferredModel: {
        provider: AIServiceProvider.Ollama,
        model: 'llama3.1',
      },
      localFallbackModel: undefined,
      onPendingChange,
      onCustomProviderChange: vi.fn(),
      onAddCustomProvider: vi.fn(),
      onRemoveCustomProvider: vi.fn(),
      onPreferredModelChange: vi.fn(),
      onFallbackModelChange: vi.fn(),
      ...temperatureProps,
    };

    const { rerender } = render(
      <AIModelsTab
        {...baseProps}
        serviceConfigs={serviceConfigs}
      />,
    );

    const input = screen.getByDisplayValue('http://127.0.0.1:11434');
    fireEvent.change(input, {
      target: { value: 'http://remote-host:11434' },
    });

    expect(onPendingChange).toHaveBeenCalledWith(AIServiceProvider.Ollama, {
      baseUrl: 'http://remote-host:11434',
    });

    rerender(
      <AIModelsTab
        {...baseProps}
        serviceConfigs={{
          ...serviceConfigs,
          [AIServiceProvider.Ollama]: {
            baseUrl: 'http://remote-host:11434',
          },
        }}
      />,
    );

    expect(screen.getByDisplayValue('http://remote-host:11434')).toBeVisible();
  });

  it('triggers onAddCustomProvider when clicking add button', () => {
    const onAddCustomProvider = vi.fn();
    const serviceConfigs: Record<AIServiceProvider, ServiceConfig> = {
      [AIServiceProvider.Groq]: {},
      [AIServiceProvider.OpenAI]: {},
      [AIServiceProvider.Anthropic]: {},
      [AIServiceProvider.Gemini]: {},
      [AIServiceProvider.Fireworks]: {},
      [AIServiceProvider.Cerebras]: {},
      [AIServiceProvider.Ollama]: {},
      [AIServiceProvider.OpenRouter]: {},
      [AIServiceProvider.Empty]: {},
    };

    render(
      <AIModelsTab
        serviceConfigs={serviceConfigs}
        customProviders={[]}
        providerEntries={[AIServiceProvider.OpenAI]}
        localPreferredModel={{
          provider: AIServiceProvider.OpenAI,
          model: 'gpt-4o',
        }}
        localFallbackModel={undefined}
        onPendingChange={vi.fn()}
        onCustomProviderChange={vi.fn()}
        onAddCustomProvider={onAddCustomProvider}
        onRemoveCustomProvider={vi.fn()}
        onPreferredModelChange={vi.fn()}
        onFallbackModelChange={vi.fn()}
        {...temperatureProps}
      />,
    );

    fireEvent.click(
      screen.getByRole('button', {
        name: 'Add Custom OpenAI Provider',
      }),
    );

    expect(onAddCustomProvider).toHaveBeenCalledTimes(1);
  });

  it('shows temperature field only when override is enabled', () => {
    const onTemperatureOverrideEnabledChange = vi.fn();
    const serviceConfigs: Record<AIServiceProvider, ServiceConfig> = {
      [AIServiceProvider.Groq]: {},
      [AIServiceProvider.OpenAI]: {},
      [AIServiceProvider.Anthropic]: {},
      [AIServiceProvider.Gemini]: {},
      [AIServiceProvider.Fireworks]: {},
      [AIServiceProvider.Cerebras]: {},
      [AIServiceProvider.Ollama]: {},
      [AIServiceProvider.OpenRouter]: {},
      [AIServiceProvider.Empty]: {},
    };

    const { rerender } = render(
      <AIModelsTab
        serviceConfigs={serviceConfigs}
        customProviders={[]}
        providerEntries={[AIServiceProvider.OpenAI]}
        localPreferredModel={{
          provider: AIServiceProvider.OpenAI,
          model: 'gpt-4o',
        }}
        localFallbackModel={undefined}
        onPendingChange={vi.fn()}
        onCustomProviderChange={vi.fn()}
        onAddCustomProvider={vi.fn()}
        onRemoveCustomProvider={vi.fn()}
        onPreferredModelChange={vi.fn()}
        onFallbackModelChange={vi.fn()}
        temperatureOverrideEnabled={false}
        temperature={0.7}
        onTemperatureOverrideEnabledChange={
          onTemperatureOverrideEnabledChange
        }
        onTemperatureChange={vi.fn()}
      />,
    );

    expect(screen.queryByPlaceholderText('0.7')).not.toBeInTheDocument();

    fireEvent.click(screen.getByLabelText('Override temperature'));
    expect(onTemperatureOverrideEnabledChange).toHaveBeenCalledWith(true);

    rerender(
      <AIModelsTab
        serviceConfigs={serviceConfigs}
        customProviders={[]}
        providerEntries={[AIServiceProvider.OpenAI]}
        localPreferredModel={{
          provider: AIServiceProvider.OpenAI,
          model: 'gpt-4o',
        }}
        localFallbackModel={undefined}
        onPendingChange={vi.fn()}
        onCustomProviderChange={vi.fn()}
        onAddCustomProvider={vi.fn()}
        onRemoveCustomProvider={vi.fn()}
        onPreferredModelChange={vi.fn()}
        onFallbackModelChange={vi.fn()}
        temperatureOverrideEnabled
        temperature={0.7}
        onTemperatureOverrideEnabledChange={
          onTemperatureOverrideEnabledChange
        }
        onTemperatureChange={vi.fn()}
      />,
    );

    expect(screen.getByDisplayValue('0.7')).toBeVisible();
  });
});
