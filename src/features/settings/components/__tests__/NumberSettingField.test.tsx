import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { NumberSettingField } from '../NumberSettingField';
import { parseIntegerInput } from '../settings-number-utils';

describe('NumberSettingField', () => {
  it('allows typing values below min until blur, then clamps', () => {
    const onValueChange = vi.fn();

    render(
      <NumberSettingField
        label="Pattern length"
        description="Minimum repeating length"
        value={256}
        min={32}
        max={1024}
        parseValue={(rawValue) =>
          parseIntegerInput(rawValue, {
            fallback: 256,
            min: 32,
            max: 1024,
          })
        }
        onValueChange={onValueChange}
      />,
    );

    const input = screen.getByRole('spinbutton');
    fireEvent.focus(input);
    fireEvent.change(input, { target: { value: '2' } });

    expect(input).toHaveValue(2);
    expect(onValueChange).not.toHaveBeenCalled();

    fireEvent.change(input, { target: { value: '256' } });
    expect(onValueChange).toHaveBeenCalledWith(256);

    fireEvent.change(input, { target: { value: '10' } });
    expect(input).toHaveValue(10);
    expect(onValueChange).toHaveBeenCalledTimes(1);

    fireEvent.blur(input);
    expect(onValueChange).toHaveBeenLastCalledWith(32);
    expect(input).toHaveValue(32);
  });
});
