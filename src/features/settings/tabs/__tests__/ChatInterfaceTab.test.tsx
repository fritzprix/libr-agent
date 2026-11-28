import { fireEvent, render, screen, within } from '@testing-library/react';
import '@testing-library/jest-dom';
import type { ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';

import { DEFAULT_SETTING } from '@/context/SettingsContext';
import ChatInterfaceTab from '../ChatInterfaceTab';
import {
  KEYBOARD_TOKEN_FAST_STEP,
  KEYBOARD_TOKEN_STEP,
  SLIDER_MAX_POSITION,
  TOKEN_UNIT,
  clampAndRoundTokenValue,
  formatTokenValue,
  sliderPositionToValue,
  valueToSliderPosition,
} from '../tokenSlider';
import { TEST_ADVANCED_SETTINGS } from '@/test/fixtures/settings';

// The real Radix Slider measures the thumb with ResizeObserver, which jsdom
// does not provide.
class ResizeObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
}
if (!('ResizeObserver' in globalThis)) {
  (
    globalThis as unknown as { ResizeObserver: unknown }
  ).ResizeObserver = ResizeObserverStub;
}

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, defaultValue?: string) => defaultValue || key,
  }),
}));

// Only Button and Input are mocked (for compact assertions and the
// data-variant check). The Slider is the real Radix component so keyboard
// behavior is exercised on the production code path.
vi.mock('@/components/ui', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@/components/ui')>();
  return {
    ...actual,
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
  };
});

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

  it('every 1K value in both ranges is reachable by keyboard steps', () => {
    const ranges: Array<[number, number]> = [
      [1024, 131072], // Max Output Tokens: 128 values
      [MIN_INPUT, MAX_INPUT], // Max Input Context: 4089 values
    ];
    for (const [min, max] of ranges) {
      let reached = 0;
      for (let value = min; value < max; value += TOKEN_UNIT) {
        // One arrow press from any 1K value lands exactly on the next one,
        // so repeated presses visit every value in the range.
        expect(
          clampAndRoundTokenValue(value + KEYBOARD_TOKEN_STEP, min, max),
        ).toBe(value + TOKEN_UNIT);
        reached += 1;
      }
      expect(reached).toBe((max - min) / TOKEN_UNIT);
      // One press past the end clamps instead of overshooting.
      expect(
        clampAndRoundTokenValue(max + KEYBOARD_TOKEN_STEP, min, max),
      ).toBe(max);
    }
    expect(KEYBOARD_TOKEN_FAST_STEP).toBe(KEYBOARD_TOKEN_STEP * 10);
  });

  it('formats token values with exact 1K labels', () => {
    expect(formatTokenValue(8192)).toBe('8K');
    expect(formatTokenValue(204800)).toBe('200K');
    expect(formatTokenValue(1048576)).toBe('1M');
    expect(formatTokenValue(4194304)).toBe('4M');
    // Non-exact megabyte values keep the exact 1K label, never a rounded "1M".
    expect(formatTokenValue(1049600)).toBe('1025K');
    expect(formatTokenValue(1572864)).toBe('1536K');
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

    const inputSlider = () =>
      screen.getByRole('slider', { name: 'Max Input Context slider' });
    const outputSlider = () =>
      screen.getByRole('slider', {
        name: 'Max Output Tokens (Default) slider',
      });

    it('shows the default as 48K and highlights only the matching preset', () => {
      renderCompact(DEFAULT_SETTING.maxInputContext);
      expect(DEFAULT_SETTING.maxInputContext).toBe(49152);
      expect(screen.getAllByText('48K').length).toBeGreaterThan(0);

      const inputGroup = screen.getByRole('group', {
        name: 'Max Input Context',
      });
      expect(
        within(inputGroup).getByRole('button', { name: '48K' }),
      ).toHaveAttribute('data-variant', 'default');
      expect(
        within(inputGroup).getByRole('button', { name: '64K' }),
      ).toHaveAttribute('data-variant', 'outline');
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

    it('labels a stored non-preset value with its exact 1K label', () => {
      const { onMaxInputContextChange } = renderCompact(1049600);
      // 1025K must not render as "1M" like the (unselected) 1M preset.
      expect(screen.getByText('1025K')).toBeInTheDocument();
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

      // The real slider thumb reflects the stored value positionally.
      const thumb = inputSlider();
      const now = Number(thumb.getAttribute('aria-valuenow'));
      const expected = valueToSliderPosition(204800, 8192, 4194304);
      expect(now).toBe(expected);
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

    it('arrow keys on the real slider step the token value by exactly 1K', () => {
      // 64K was a stuck value before: one track position step rounded back to
      // 64K, so the thumb never moved.
      const { onMaxInputContextChange } = renderCompact(65536, 65536);

      fireEvent.keyDown(inputSlider(), { key: 'ArrowRight' });
      expect(onMaxInputContextChange).toHaveBeenLastCalledWith(65536 + 1024);

      fireEvent.keyDown(inputSlider(), { key: 'ArrowDown' });
      expect(onMaxInputContextChange).toHaveBeenLastCalledWith(65536 - 1024);

      fireEvent.keyDown(inputSlider(), { key: 'ArrowLeft' });
      expect(onMaxInputContextChange).toHaveBeenLastCalledWith(65536 - 1024);

      fireEvent.keyDown(inputSlider(), { key: 'PageUp' });
      expect(onMaxInputContextChange).toHaveBeenLastCalledWith(65536 + 10240);

      fireEvent.keyDown(inputSlider(), { key: 'ArrowRight', shiftKey: true });
      expect(onMaxInputContextChange).toHaveBeenLastCalledWith(65536 + 10240);
    });

    it('reaches values near the top of the range that track positions skipped', () => {
      const { onMaxInputContextChange } = renderCompact(4194304);

      // Neighbouring positions near 4M were about 26K apart; one ArrowLeft
      // press must step exactly 1K below the maximum.
      fireEvent.keyDown(inputSlider(), { key: 'ArrowLeft' });
      expect(onMaxInputContextChange).toHaveBeenLastCalledWith(4194304 - 1024);

      // At the maximum, an up press is clamped and emits nothing new.
      onMaxInputContextChange.mockClear();
      fireEvent.keyDown(inputSlider(), { key: 'ArrowRight' });
      expect(onMaxInputContextChange).not.toHaveBeenCalled();
    });

    it('Home and End jump the slider to the range ends', () => {
      const { onMaxInputContextChange } = renderCompact(49152);

      fireEvent.keyDown(inputSlider(), { key: 'End' });
      expect(onMaxInputContextChange).toHaveBeenLastCalledWith(4194304);

      fireEvent.keyDown(inputSlider(), { key: 'Home' });
      expect(onMaxInputContextChange).toHaveBeenLastCalledWith(8192);
    });

    it('keyboard steps work on the output tokens slider too', () => {
      const { onAdvancedSettingsChange } = renderCompact(49152, 1024);

      // 1K was stuck before as well.
      fireEvent.keyDown(outputSlider(), { key: 'ArrowRight' });
      expect(onAdvancedSettingsChange).toHaveBeenLastCalledWith(
        'defaultMaxOutputTokens',
        2048,
      );
    });
  });
});
