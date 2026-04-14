import { fireEvent, render, screen } from '@testing-library/react';
import '@testing-library/jest-dom';
import { describe, expect, it, vi } from 'vitest';

import { AIServiceProvider } from '@/lib/ai-service';
import { ProviderCard } from '../ProviderCard';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, defaultValue?: string) => defaultValue || key,
  }),
}));

describe('ProviderCard', () => {
  it('pushes base URL changes immediately to the pending form state', () => {
    const onPendingChange = vi.fn();

    render(
      <ProviderCard
        provider={AIServiceProvider.Ollama}
        providerName="Ollama"
        apiKey=""
        baseUrl="http://127.0.0.1:11434"
        onPendingChange={onPendingChange}
      />,
    );

    fireEvent.change(screen.getByDisplayValue('http://127.0.0.1:11434'), {
      target: { value: 'http://remote-host:11434' },
    });

    expect(onPendingChange).toHaveBeenCalledWith(AIServiceProvider.Ollama, {
      baseUrl: 'http://remote-host:11434',
    });
  });

  it('reflects updated props without keeping stale local input state', () => {
    const onPendingChange = vi.fn();

    const { rerender } = render(
      <ProviderCard
        provider={AIServiceProvider.Ollama}
        providerName="Ollama"
        apiKey=""
        baseUrl="http://127.0.0.1:11434"
        onPendingChange={onPendingChange}
      />,
    );

    rerender(
      <ProviderCard
        provider={AIServiceProvider.Ollama}
        providerName="Ollama"
        apiKey=""
        baseUrl="http://another-host:11434"
        onPendingChange={onPendingChange}
      />,
    );

    expect(screen.getByDisplayValue('http://another-host:11434')).toBeVisible();
  });
});
