import { fireEvent, render, screen } from '@testing-library/react';
import '@testing-library/jest-dom';
import type { ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';

import { DEFAULT_SETTING } from '@/context/SettingsContext';
import ChatInterfaceTab from '../ChatInterfaceTab';
import { TEST_ADVANCED_SETTINGS } from '@/test/fixtures/settings';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, defaultValue?: string) => defaultValue || key,
  }),
}));

vi.mock('@/components/ui', () => ({
  Button: ({
    children,
    onClick,
    disabled,
    ...props
  }: {
    children: ReactNode;
    onClick?: () => void;
    disabled?: boolean;
  }) => (
    <button onClick={onClick} disabled={disabled} {...props}>
      {children}
    </button>
  ),
  Input: ({
    value,
    onChange,
    ...props
  }: {
    value?: string | number;
    onChange?: (event: React.ChangeEvent<HTMLInputElement>) => void;
  }) => <input value={value} onChange={onChange} {...props} />,
  Slider: () => <div data-testid="mock-slider" />,
}));

describe('ChatInterfaceTab', () => {
  it('rerenders when context strategy changes', () => {
    const onContextStrategyChange = vi.fn();
    const onWindowSizeChange = vi.fn();
    const onMaxInputContextChange = vi.fn();
    const onToolCallGroupVisibleCountChange = vi.fn();
    const onAdvancedSettingsChange = vi.fn();

    const { rerender } = render(
      <ChatInterfaceTab
        localContextStrategy="window"
        localWindowSize={DEFAULT_SETTING.windowSize}
        localMaxInputContext={DEFAULT_SETTING.maxInputContext}
        localToolCallGroupVisibleCount={DEFAULT_SETTING.toolCallGroupVisibleCount}
        localAdvancedSettings={TEST_ADVANCED_SETTINGS}
        onContextStrategyChange={onContextStrategyChange}
        onWindowSizeChange={onWindowSizeChange}
        onMaxInputContextChange={onMaxInputContextChange}
        onToolCallGroupVisibleCountChange={onToolCallGroupVisibleCountChange}
        onAdvancedSettingsChange={onAdvancedSettingsChange}
      />,
    );

    const slidingWindowButton = screen.getByRole('radio', {
      name: /Sliding Window/,
    });
    const compactButton = screen.getByRole('radio', {
      name: /Compact/,
    });

    expect(slidingWindowButton).toHaveAttribute('aria-checked', 'true');
    expect(compactButton).toHaveAttribute('aria-checked', 'false');

    fireEvent.click(compactButton);
    expect(onContextStrategyChange).toHaveBeenCalledWith('compact');

    rerender(
      <ChatInterfaceTab
        localContextStrategy="compact"
        localWindowSize={DEFAULT_SETTING.windowSize}
        localMaxInputContext={DEFAULT_SETTING.maxInputContext}
        localToolCallGroupVisibleCount={DEFAULT_SETTING.toolCallGroupVisibleCount}
        localAdvancedSettings={TEST_ADVANCED_SETTINGS}
        onContextStrategyChange={onContextStrategyChange}
        onWindowSizeChange={onWindowSizeChange}
        onMaxInputContextChange={onMaxInputContextChange}
        onToolCallGroupVisibleCountChange={onToolCallGroupVisibleCountChange}
        onAdvancedSettingsChange={onAdvancedSettingsChange}
      />,
    );

    expect(slidingWindowButton).toHaveAttribute('aria-checked', 'false');
    expect(compactButton).toHaveAttribute('aria-checked', 'true');
  });

  describe('max input context (compact strategy)', () => {
    const renderCompact = (maxInputContext: number) => {
      const onMaxInputContextChange = vi.fn();
      render(
        <ChatInterfaceTab
          localContextStrategy="compact"
          localWindowSize={DEFAULT_SETTING.windowSize}
          localMaxInputContext={maxInputContext}
          localToolCallGroupVisibleCount={
            DEFAULT_SETTING.toolCallGroupVisibleCount
          }
          localAdvancedSettings={TEST_ADVANCED_SETTINGS}
          onContextStrategyChange={vi.fn()}
          onWindowSizeChange={vi.fn()}
          onMaxInputContextChange={onMaxInputContextChange}
          onToolCallGroupVisibleCountChange={vi.fn()}
          onAdvancedSettingsChange={vi.fn()}
        />,
      );
      return onMaxInputContextChange;
    };

    it('shows the default as 48K', () => {
      renderCompact(DEFAULT_SETTING.maxInputContext);
      expect(DEFAULT_SETTING.maxInputContext).toBe(49152);
      expect(screen.getAllByText('48K').length).toBeGreaterThan(0);
    });

    it('can select 1M and the top preset, and reports the exact value', () => {
      const onMaxInputContextChange = renderCompact(
        DEFAULT_SETTING.maxInputContext,
      );

      fireEvent.click(screen.getByRole('button', { name: '1M' }));
      expect(onMaxInputContextChange).toHaveBeenLastCalledWith(1048576);

      fireEvent.click(screen.getByRole('button', { name: '4M' }));
      expect(onMaxInputContextChange).toHaveBeenLastCalledWith(4194304);
    });

    it('labels large values in M and keeps a stored non-preset value as is', () => {
      const onMaxInputContextChange = renderCompact(1572864);
      expect(screen.getAllByText('1.5M').length).toBeGreaterThan(0);
      // Showing it does not save anything.
      expect(onMaxInputContextChange).not.toHaveBeenCalled();
    });
  });
});
