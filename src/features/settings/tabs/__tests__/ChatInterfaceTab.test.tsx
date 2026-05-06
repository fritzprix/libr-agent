import { fireEvent, render, screen } from '@testing-library/react';
import '@testing-library/jest-dom';
import type { ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';

import { DEFAULT_SETTING } from '@/context/SettingsContext';
import ChatInterfaceTab from '../ChatInterfaceTab';

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
        localAdvancedSettings={DEFAULT_SETTING.advanced}
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
        localAdvancedSettings={DEFAULT_SETTING.advanced}
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
});
