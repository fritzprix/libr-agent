import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  applyAfterInputFallback,
  collectInspectableElements,
  labelOf,
  rectOf,
  resolveTarget,
} from '../e2e-ui-bridge';

describe('e2e-ui-bridge helpers (#2902)', () => {
  afterEach(() => {
    document.body.innerHTML = '';
    vi.restoreAllMocks();
  });

  it('rounds getBoundingClientRect coordinates to integers for Rust ViewportRect', () => {
    const btn = document.createElement('button');
    vi.spyOn(btn, 'getBoundingClientRect').mockReturnValue({
      x: 12.6,
      y: 34.2,
      width: 100.8,
      height: 29.4,
      top: 34.2,
      left: 12.6,
      right: 113.4,
      bottom: 63.6,
      toJSON: () => ({}),
    });
    expect(rectOf(btn)).toEqual({ x: 13, y: 34, width: 101, height: 29 });
  });

  it('never leaks a password input value in labelOf or collectInspectableElements', () => {
    document.body.innerHTML = `
      <label for="user-prompt-input">LIBRAGENT_EMAIL_PASSWORD</label>
      <input id="user-prompt-input" type="password" value="dummy-token-abcd" />
    `;
    const input = document.getElementById('user-prompt-input')!;
    expect(labelOf(input)).toBe('LIBRAGENT_EMAIL_PASSWORD');
    expect(labelOf(input)).not.toContain('dummy-token-abcd');

    const elements = collectInspectableElements('#user-prompt-input');
    expect(elements).toHaveLength(1);
    expect(elements[0].inputType).toBe('password');
    expect(JSON.stringify(elements)).not.toContain('dummy-token-abcd');
  });

  it('resolves targets by selector or data-testid and applies fallback type/scroll', () => {
    document.body.innerHTML = `
      <input id="secret-field" data-testid="secret-input" type="password" value="" />
      <div id="scroller" style="height: 100px; overflow-y: auto;"></div>
    `;
    const input = resolveTarget({
      requestId: 'req-1',
      testId: 'secret-input',
    }) as HTMLInputElement;
    expect(input).not.toBeNull();
    expect(input.id).toBe('secret-field');

    input.focus();
    applyAfterInputFallback({
      action: 'type',
      text: 'dummy-token-abcd',
    });
    expect(input.value).toBe('dummy-token-abcd');
  });
});
