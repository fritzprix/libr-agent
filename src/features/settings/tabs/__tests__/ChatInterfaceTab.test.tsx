import { fireEvent, render, screen } from '@testing-library/react';
import '@testing-library/jest-dom';
import type { ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';

import { DEFAULT_SETTING } from '@/context/SettingsContext';
import ChatInterfaceTab from '../ChatInterfaceTab';
import {
  SLIDER_MAX_POSITION,
  TOKEN_UNIT,
  clampAndRoundTokenValue,
  formatTokenValue,
  sliderPositionToValue,
  valueToSliderPosition,
} from '../tokenSlider';
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
    variant,
    ...props
  }: {
    children: ReactNode;
    onClick?: () => void;
    disabled?: boolean;
    variant?: string;
  }) => (
    <button
      onClick={onClick}
      disabled={disabled}
      data-variant={variant}
      {...props}
    >
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
  Slider: ({
    value,
    min,
    max,
    step,
    onValueChange,
    'aria-label': ariaLabel,
  }: {
    value?: number[];
    min?: number;
    max?: number;
    step?: number;
    onValueChange?: (values: number[]) => void;
    'aria-label'?: string;
  }) => (
    <input
      type="range"
      aria-label={`${ariaLabel ?? 'Slider'} slider`}
      min={min}
      max={max}
      step={step}
      value={value?.[0] ?? 0}
      onChange={(e) => onValueChange?.([Number(e.target.value)])}
    />
  ),
}));

describe('tokenSlider helpers', () => {
  const MIN_INPUT = 8192;
  const MAX_INPUT = 4194304;

  it('rounds to nearest 1K and clamps to range', () => {
    expect(clampAndRoundTokenValue(204800, MIN_INPUT, MAX_INPUT)).toBe(204800);
    expect(clampAndRoundTokenValue(200000, MIN_INPUT, MAX_INPUT)).toBe(199680);
    expect(clampAndRoundTokenValue(1000, MIN_INPUT, MAX_INPUT)).toBe(MIN_INPUT);
    expect(clampAndRoundTokenValue(9999999, MIN_INPUT, MAX_INPUT)).toBe(
      MAX_INPUT,
    );
    expect(clampAndRoundTokenValue(Number.NaN, MIN_INPUT, MAX_INPUT)).toBe(
      MIN_INPUT,
    );
  });

  it('maps endpoints and intermediate values logarithmically', () => {
    expect(valueToSliderPosition(MIN_INPUT, MIN_INPUT, MAX_INPUT)).toBe(0);
    expect(valueToSliderPosition(MAX_INPUT, MIN_INPUT, MAX_INPUT)).toBe(
      SLIDER_MAX_POSITION,
    );
    expect(sliderPositionToValue(0, MIN_INPUT, MAX_INPUT)).toBe(MIN_INPUT);
    expect(
      sliderPositionToValue(SLIDER_MAX_POSITION, MIN_INPUT, MAX_INPUT),
    ).toBe(MAX_INPUT);

    // 8K to 64K spans 3 doublings out of 9 (8K -> 4M), so ~33.3% of the slider track
    const pos64k = valueToSliderPosition(65536, MIN_INPUT, MAX_INPUT);
    expect(pos64k).toBeGreaterThan(300);
    expect(pos64k).toBeLessThan(360);

    // Round-trip a non-preset value like 200K (204800)
    const pos200k = valueToSliderPosition(204800, MIN_INPUT, MAX_INPUT);
    const back200k = sliderPositionToValue(pos200k, MIN_INPUT, MAX_INPUT);
    expect(back200k % TOKEN_UNIT).toBe(0);
    expect(Math.abs(back200k - 204800)).toBeLessThanOrEqual(TOKEN_UNIT);
  });

  it('formats token values in K and M', () => {
    expect(formatTokenValue(8192)).toBe('8K');
    expect(formatTokenValue(204800)).toBe('200K');
    expect(formatTokenValue(1048576)).toBe('1M');
    expect(formatTokenValue(1572864)).toBe('1.5M');
  });
});

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

  describe('token sliders (max output tokens and max input context)', () => {
    const renderCompact = (
      maxInputContext: number,
      defaultMaxOutputTokens: number = TEST_ADVANCED_SETTINGS.defaultMaxOutputTokens,
    ) => {
      const onMaxInputContextChange = vi.fn();
      const onAdvancedSettingsChange = vi.fn();
      const result = render(
        <ChatInterfaceTab
          localContextStrategy="compact"
          localWindowSize={DEFAULT_SETTING.windowSize}
          localMaxInputContext={maxInputContext}
          localToolCallGroupVisibleCount={
            DEFAULT_SETTING.toolCallGroupVisibleCount
          }
          localAdvancedSettings={{
            ...TEST_ADVANCED_SETTINGS,
            defaultMaxOutputTokens,
          }}
          onContextStrategyChange={vi.fn()}
          onWindowSizeChange={vi.fn()}
          onMaxInputContextChange={onMaxInputContextChange}
          onToolCallGroupVisibleCountChange={vi.fn()}
          onAdvancedSettingsChange={onAdvancedSettingsChange}
        />,
      );
      return { onMaxInputContextChange, onAdvancedSettingsChange, ...result };
    };

    it('shows the default as 48K and highlights only the matching preset', () => {
      renderCompact(DEFAULT_SETTING.maxInputContext);
      expect(DEFAULT_SETTING.maxInputContext).toBe(49152);
      expect(screen.getAllByText('48K').length).toBeGreaterThan(0);

      const preset48k = screen.getByRole('button', { name: '48K' });
      const preset64k = screen.getAllByRole('button', { name: '64K' })[1];
      expect(preset48k).toHaveAttribute('data-variant', 'default');
      expect(preset64k).toHaveAttribute('data-variant', 'outline');
    });

    it('can select 1M and the top preset, and reports the exact value', () => {
      const { onMaxInputContextChange } = renderCompact(
        DEFAULT_SETTING.maxInputContext,
      );

      fireEvent.click(screen.getByRole('button', { name: '1M' }));
      expect(onMaxInputContextChange).toHaveBeenLastCalledWith(1048576);

      fireEvent.click(screen.getByRole('button', { name: '4M' }));
      expect(onMaxInputContextChange).toHaveBeenLastCalledWith(4194304);
    });

    it('labels large values in M and keeps a stored non-preset value as is', () => {
      const { onMaxInputContextChange } = renderCompact(1572864);
      expect(screen.getAllByText('1.5M').length).toBeGreaterThan(0);
      expect(onMaxInputContextChange).not.toHaveBeenCalled();
    });

    it('displays a saved non-preset value (200K) in label, input, slider, with no preset selected', () => {
      renderCompact(204800, 12288);

      expect(screen.getByText('200K')).toBeInTheDocument();
      expect(screen.getByText('12K')).toBeInTheDocument();

      const inputContextField = screen.getByRole('spinbutton', {
        name: 'Max Input Context',
      });
      expect(inputContextField).toHaveValue(204800);

      const outputTokensField = screen.getByRole('spinbutton', {
        name: 'Max Output Tokens (Default)',
      });
      expect(outputTokensField).toHaveValue(12288);

      // No preset button should be selected since neither 204800 nor 12288 is a preset
      const presetButtons = screen
        .getAllByRole('button')
        .filter((btn) => btn.getAttribute('data-variant') === 'default');
      expect(presetButtons).toHaveLength(0);
    });

    it('rounds and clamps typed values in the number input on blur and Enter', () => {
      const { onMaxInputContextChange, onAdvancedSettingsChange } =
        renderCompact(49152, 8192);

      const inputContextField = screen.getByRole('spinbutton', {
        name: 'Max Input Context',
      });
      fireEvent.change(inputContextField, { target: { value: '200000' } });
      fireEvent.blur(inputContextField);
      expect(onMaxInputContextChange).toHaveBeenLastCalledWith(199680);

      // Clamping above max on Enter
      fireEvent.change(inputContextField, { target: { value: '9999999' } });
      fireEvent.keyDown(inputContextField, { key: 'Enter' });
      expect(onMaxInputContextChange).toHaveBeenLastCalledWith(4194304);

      // Max Output Tokens typed input clamping below min
      const outputTokensField = screen.getByRole('spinbutton', {
        name: 'Max Output Tokens (Default)',
      });
      fireEvent.change(outputTokensField, { target: { value: '500' } });
      fireEvent.blur(outputTokensField);
      expect(onAdvancedSettingsChange).toHaveBeenLastCalledWith(
        'defaultMaxOutputTokens',
        1024,
      );
    });

    it('updates token value in 1K steps when moving the slider', () => {
      const { onMaxInputContextChange, onAdvancedSettingsChange } =
        renderCompact(49152, 8192);

      const inputSlider = screen.getByRole('slider', {
        name: 'Max Input Context slider',
      });
      fireEvent.change(inputSlider, { target: { value: '500' } });
      expect(onMaxInputContextChange).toHaveBeenCalledTimes(1);
      const emittedInput = onMaxInputContextChange.mock.calls[0][0];
      expect(emittedInput % TOKEN_UNIT).toBe(0);
      expect(emittedInput).toBeGreaterThanOrEqual(8192);
      expect(emittedInput).toBeLessThanOrEqual(4194304);

      const outputSlider = screen.getByRole('slider', {
        name: 'Max Output Tokens (Default) slider',
      });
      fireEvent.change(outputSlider, { target: { value: '500' } });
      expect(onAdvancedSettingsChange).toHaveBeenCalledTimes(1);
      const [key, emittedOutput] = onAdvancedSettingsChange.mock.calls[0];
      expect(key).toBe('defaultMaxOutputTokens');
      expect(emittedOutput % TOKEN_UNIT).toBe(0);
      expect(emittedOutput).toBeGreaterThanOrEqual(1024);
      expect(emittedOutput).toBeLessThanOrEqual(131072);
    });
  });
});
