import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  collectInspectableElements,
  dispatchDomInput,
  labelOf,
  pointerButton,
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

  it('resolves targets by selector or data-testid', () => {
    document.body.innerHTML = `
      <input id="secret-field" data-testid="secret-input" type="password" value="" />
    `;
    const input = resolveTarget({
      requestId: 'req-1',
      testId: 'secret-input',
    }) as HTMLInputElement;
    expect(input).not.toBeNull();
    expect(input.id).toBe('secret-field');
  });

  it('dispatches element input as DOM events in-webview (#2902 owner ruling)', () => {
    document.body.innerHTML = `
      <input id="secret-field" type="text" value="pre" />
      <button id="btn">Go</button>
      <div id="scroller" data-testid="scroller" style="height: 100px; overflow-y: auto;"></div>
    `;
    const input = document.getElementById('secret-field') as HTMLInputElement;
    const btn = document.getElementById('btn')!;
    const captured: { click?: MouseEvent; contextmenu?: MouseEvent; keydown?: KeyboardEvent } = {};

    // type: appends through the React-compatible native setter with input events.
    let sawInput = false;
    input.addEventListener('input', () => {
      sawInput = true;
    });
    const typeDesc = dispatchDomInput(input, {
      requestId: 'r',
      action: 'type',
      text: 'dummy-token-abcd',
    });
    expect(typeDesc).toContain('type:');
    expect(input.value).toBe('predummy-token-abcd');
    expect(sawInput).toBe(true);

    // click: full pointer+mouse sequence reaching the click handler.
    let clicks = 0;
    btn.addEventListener('click', (e) => {
      clicks += 1;
      captured.click = e as MouseEvent;
    });
    const clickDesc = dispatchDomInput(btn, { requestId: 'r', action: 'click' });
    expect(clickDesc).toContain('click');
    expect(clicks).toBe(1);
    expect(captured.click?.button).toBe(0);

    // right_click: contextmenu with button=2.
    btn.addEventListener('contextmenu', (e) => {
      captured.contextmenu = e as MouseEvent;
    });
    dispatchDomInput(btn, { requestId: 'r', action: 'right_click' });
    expect(captured.contextmenu?.button).toBe(2);

    // scroll: scrollable container moves and emits a scroll event.
    const scroller = document.querySelector(
      '[data-testid="scroller"]',
    ) as HTMLElement;
    Object.defineProperty(scroller, 'scrollHeight', {
      value: 1000,
      configurable: true,
    });
    Object.defineProperty(scroller, 'clientHeight', {
      value: 100,
      configurable: true,
    });
    let scrollEvents = 0;
    scroller.addEventListener('scroll', () => {
      scrollEvents += 1;
    });
    dispatchDomInput(scroller, { requestId: 'r', action: 'scroll', scrollAmount: 5 });
    expect(scroller.scrollTop).toBe(600);
    expect(scrollEvents).toBe(1);

    // key: keydown/keyup with modifier flags on the target element.
    input.addEventListener('keydown', (e) => {
      captured.keydown = e as KeyboardEvent;
    });
    dispatchDomInput(input, {
      requestId: 'r',
      action: 'key',
      key: 'Enter',
      modifiers: ['Ctrl'],
    });
    expect(captured.keydown?.key).toBe('Enter');
    expect(captured.keydown?.ctrlKey).toBe(true);

    expect(pointerButton('middle_click')).toBe(1);
    expect(pointerButton('click')).toBe(0);
  });
});
