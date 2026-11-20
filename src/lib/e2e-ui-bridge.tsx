/**
 * E2E UI bridge (#2902) — the webview half of the debug-only UI automation
 * control plane (`/api/e2e-control/ui/*`).
 *
 * The Rust side emits Tauri events (`e2e-navigate`, `e2e-inspect`,
 * `e2e-input`); this module answers them inside the running UI:
 *
 * - navigate: push a router path and/or open an agent side panel,
 * - inspect: enumerate interactable elements (role / label / test id /
 *   viewport bounds) so an external agent can pick targets without guessing,
 * - input: resolve one selector / test id and dispatch the action as DOM
 *   events inside this webview — the default element-input path per the
 *   #2902 owner ruling; OS-level input is never used for these.
 *
 * Answers travel back through the `e2e_ui_report` Tauri command, which is
 * inert unless the control plane is enabled. The whole bridge stays
 * dormant in release builds: `e2e_ui_enabled` answers false there, so no
 * listeners are ever registered.
 */
import { useEffect, useRef } from 'react';
import { useNavigate } from 'react-router-dom';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';

/** True after the first successful `e2e_ui_enabled` ping in this document. */
let bridgeActive = false;

/** Pending open dialog count tracked by mounted DialogContent instances. */
let mountedDialogCount = 0;

/**
 * Report a shared-Dialog open/close transition (no-op unless the bridge is
 * active). Called from `DialogContent` mount/unmount so both controlled
 * (`<Dialog open={...}>`) and trigger-driven dialogs are tracked accurately.
 */
export function reportDialogOpenChange(open: boolean): void {
  if (open) {
    mountedDialogCount += 1;
  } else {
    mountedDialogCount = Math.max(0, mountedDialogCount - 1);
  }
  if (!bridgeActive) return;
  invoke('e2e_ui_report_dialog', { open }).catch(() => {
    /* bridge raced away — nothing to report to */
  });
}

export const E2E_PANEL_EVENT = 'libragent:e2e-panel';

declare global {
  interface Window {
    __LIBRAGENT_E2E_PENDING_PANEL?: string | null;
  }
}

const INTERACTIVE_SELECTOR = [
  'a[href]',
  'button',
  'input',
  'textarea',
  'select',
  '[role="button"]',
  '[role="link"]',
  '[role="tab"]',
  '[role="menuitem"]',
  '[role="menuitemcheckbox"]',
  '[role="menuitemradio"]',
  '[role="checkbox"]',
  '[role="switch"]',
  '[role="combobox"]',
  '[role="slider"]',
  '[role="textbox"]',
  '[role="dialog"]',
  '[role="region"]',
  '[data-testid]',
  '[contenteditable="true"]',
  '[tabindex]:not([tabindex="-1"])',
].join(',');

export interface ViewportRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export function rectOf(el: Element): ViewportRect {
  const r = el.getBoundingClientRect();
  return {
    x: Math.round(r.x),
    y: Math.round(r.y),
    width: Math.max(0, Math.round(r.width)),
    height: Math.max(0, Math.round(r.height)),
  };
}

export function implicitRole(el: Element): string {
  const declared = el.getAttribute('role');
  if (declared) return declared;
  const tag = el.tagName.toLowerCase();
  if (tag === 'a') return el.hasAttribute('href') ? 'link' : 'generic';
  if (tag === 'button') return 'button';
  if (tag === 'select') return 'combobox';
  if (tag === 'textarea') return 'textbox';
  if (tag === 'input') {
    const type = (el.getAttribute('type') || 'text').toLowerCase();
    if (['checkbox', 'radio'].includes(type)) return type;
    if (['button', 'submit', 'reset', 'image'].includes(type)) return 'button';
    if (['range'].includes(type)) return 'slider';
    return 'textbox';
  }
  return 'generic';
}

export function labelOf(el: Element): string {
  const aria = el.getAttribute('aria-label');
  if (aria) return aria.trim();
  if (el instanceof HTMLInputElement) {
    // Never read `.value` of a password input — captures and inspect output
    // must never expose secret characters (#2902).
    if ((el.type || '').toLowerCase() === 'password') {
      if (el.id) {
        const assoc = document.querySelector(
          `label[for="${CSS.escape(el.id)}"]`,
        );
        if (assoc?.textContent) return assoc.textContent.trim().slice(0, 80);
      }
      return el.getAttribute('placeholder')?.trim() || '';
    }
    const placeholder = el.getAttribute('placeholder');
    if (placeholder) return placeholder.trim();
    return (el.value || '').slice(0, 40);
  }
  const text = (el.textContent || '').trim().replace(/\s+/g, ' ');
  if (text) return text.slice(0, 120);
  const placeholder = el.getAttribute('placeholder');
  if (placeholder) return placeholder.trim();
  const title = el.getAttribute('title');
  if (title) return title.trim();
  return '';
}

export interface InspectEvent {
  requestId: string;
  selector?: string | null;
}

export function collectInspectableElements(selector?: string | null) {
  let scope: Element[] = [];
  try {
    scope = Array.from(
      document.querySelectorAll(selector || INTERACTIVE_SELECTOR),
    );
  } catch {
    scope = [];
  }
  return scope.map((el) => {
    const bounds = rectOf(el);
    return {
      role: implicitRole(el),
      tag: el.tagName.toLowerCase(),
      label: labelOf(el),
      id: el.id || undefined,
      testId: el.getAttribute('data-testid') || undefined,
      inputType:
        el instanceof HTMLInputElement
          ? (el.getAttribute('type') || 'text').toLowerCase()
          : undefined,
      disabled:
        el instanceof HTMLButtonElement ||
        el instanceof HTMLInputElement ||
        el instanceof HTMLSelectElement ||
        el instanceof HTMLTextAreaElement
          ? el.disabled
          : el.getAttribute('aria-disabled') === 'true',
      bounds,
      rect: bounds,
      visible: bounds.width > 0 && bounds.height > 0,
    };
  });
}

async function handleInspect(detail: InspectEvent): Promise<void> {
  const elements = collectInspectableElements(detail.selector);
  await invoke('e2e_ui_report', {
    requestId: detail.requestId,
    payload: { elements },
  }).catch(() => {});
}

export interface InputEvent {
  requestId: string;
  selector?: string | null;
  testId?: string | null;
  action?: string | null;
  text?: string | null;
  key?: string | null;
  modifiers?: string[];
  scrollAmount?: number | null;
  axis?: string | null;
  button?: string | null;
}

export function resolveTarget(detail: InputEvent): Element | null {
  const candidates: Element[] = [];
  if (detail.selector) {
    try {
      candidates.push(
        ...Array.from(document.querySelectorAll(detail.selector)),
      );
    } catch {
      /* invalid selector: fall through to testId */
    }
  }
  if (candidates.length === 0 && detail.testId) {
    candidates.push(
      ...Array.from(
        document.querySelectorAll(
          `[data-testid="${CSS.escape(detail.testId)}"]`,
        ),
      ),
    );
  }
  return (
    candidates.find((el) => {
      const r = el.getBoundingClientRect();
      return r.width > 0 && r.height > 0;
    }) ??
    candidates[0] ??
    null
  );
}

function findScrollableContainer(el: Element): HTMLElement | null {
  if (el instanceof HTMLElement && el.scrollHeight > el.clientHeight) {
    return el;
  }
  const childViewport = el.querySelector('[data-radix-scroll-area-viewport]');
  if (childViewport instanceof HTMLElement) {
    return childViewport;
  }
  const descendant = Array.from(el.querySelectorAll('*')).find(
    (node): node is HTMLElement =>
      node instanceof HTMLElement && node.scrollHeight > node.clientHeight,
  );
  if (descendant) {
    return descendant;
  }
  const ancestorViewport = el.closest('[data-radix-scroll-area-viewport]');
  if (ancestorViewport instanceof HTMLElement) {
    return ancestorViewport;
  }
  return el instanceof HTMLElement ? el : null;
}

function setReactInputValue(
  el: HTMLInputElement | HTMLTextAreaElement,
  nextValue: string,
): void {
  const proto =
    el instanceof HTMLTextAreaElement
      ? HTMLTextAreaElement.prototype
      : HTMLInputElement.prototype;
  const descriptor = Object.getOwnPropertyDescriptor(proto, 'value');
  if (descriptor?.set) {
    descriptor.set.call(el, nextValue);
  } else {
    el.value = nextValue;
  }
  el.dispatchEvent(new Event('input', { bubbles: true }));
  el.dispatchEvent(new Event('change', { bubbles: true }));
}

function modifierFlags(detail: Pick<InputEvent, 'modifiers'>) {
  const mods = (detail.modifiers || []).map((m) => m.toLowerCase());
  return {
    ctrlKey: mods.includes('ctrl') || mods.includes('control'),
    altKey: mods.includes('alt') || mods.includes('option'),
    shiftKey: mods.includes('shift'),
    metaKey:
      mods.includes('meta') ||
      mods.includes('cmd') ||
      mods.includes('command') ||
      mods.includes('super'),
  };
}

/** Button index of a pointer action (0 left, 1 middle, 2 right). */
export function pointerButton(action: string): number {
  if (action === 'right_click') return 2;
  if (action === 'middle_click') return 1;
  return 0;
}

function mouseInit(
  detail: InputEvent,
  button: number,
  buttons: number,
): MouseEventInit {
  return {
    bubbles: true,
    cancelable: true,
    button,
    buttons,
    ...modifierFlags(detail),
  };
}

function pointerInit(
  detail: InputEvent,
  button: number,
  buttons: number,
): PointerEventInit {
  return {
    ...mouseInit(detail, button, buttons),
    pointerId: 1,
    pointerType: 'mouse',
    isPrimary: true,
  };
}

/**
 * Pointer events where the webview supports them, otherwise the plain
 * MouseEvent equivalent so dispatch never throws on older engines (tests).
 */
function newPointerEvent(type: string, init: PointerEventInit): Event {
  if (typeof PointerEvent !== 'undefined') {
    return new PointerEvent(type, init);
  }
  return new MouseEvent(type, init);
}

/**
 * Dispatch one `ui/input` action to `el` as DOM events inside this webview —
 * the default element-input path (#2902 owner ruling: OS-level input is
 * never used for element targets). Returns a short description of what was
 * dispatched.
 */
export function dispatchDomInput(el: Element, detail: InputEvent): string {
  const action = (detail.action || '').toLowerCase();
  const target = el as HTMLElement;

  if (action === 'move' || action === 'hover') {
    target.dispatchEvent(
      newPointerEvent('pointerover', pointerInit(detail, 0, 0)),
    );
    target.dispatchEvent(new MouseEvent('mouseover', mouseInit(detail, 0, 0)));
    target.dispatchEvent(
      newPointerEvent('pointermove', pointerInit(detail, 0, 0)),
    );
    target.dispatchEvent(new MouseEvent('mousemove', mouseInit(detail, 0, 0)));
    return 'move: pointerover,mouseover,pointermove,mousemove';
  }

  if (
    action === 'click' ||
    action === 'double_click' ||
    action === 'right_click' ||
    action === 'middle_click'
  ) {
    const button = pointerButton(action);
    const pressed = button === 1 ? 4 : button === 2 ? 2 : 1;
    if (typeof target.focus === 'function') target.focus();
    target.dispatchEvent(
      newPointerEvent('pointerdown', pointerInit(detail, button, pressed)),
    );
    target.dispatchEvent(
      new MouseEvent('mousedown', mouseInit(detail, button, pressed)),
    );
    target.dispatchEvent(
      newPointerEvent('pointerup', pointerInit(detail, button, 0)),
    );
    target.dispatchEvent(
      new MouseEvent('mouseup', mouseInit(detail, button, 0)),
    );
    if (action === 'right_click') {
      target.dispatchEvent(
        new MouseEvent('contextmenu', mouseInit(detail, button, 0)),
      );
      return 'right_click: pointerdown,mousedown,pointerup,mouseup,contextmenu';
    }
    target.dispatchEvent(new MouseEvent('click', mouseInit(detail, button, 0)));
    if (action === 'double_click') {
      target.dispatchEvent(
        newPointerEvent('pointerdown', pointerInit(detail, button, pressed)),
      );
      target.dispatchEvent(
        new MouseEvent('mousedown', mouseInit(detail, button, pressed)),
      );
      target.dispatchEvent(
        newPointerEvent('pointerup', pointerInit(detail, button, 0)),
      );
      target.dispatchEvent(
        new MouseEvent('mouseup', mouseInit(detail, button, 0)),
      );
      target.dispatchEvent(
        new MouseEvent('click', { ...mouseInit(detail, button, 0), detail: 2 }),
      );
      target.dispatchEvent(
        new MouseEvent('dblclick', {
          ...mouseInit(detail, button, 0),
          detail: 2,
        }),
      );
      return 'double_click: (pointerdown,mousedown,pointerup,mouseup,click) x2,dblclick';
    }
    return `${action}: pointerdown,mousedown,pointerup,mouseup,click`;
  }

  if (action === 'type') {
    const text = detail.text || '';
    if (el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement) {
      setReactInputValue(el, el.value + text);
      return `type: input+change (${text.length} chars appended)`;
    }
    if (target.isContentEditable) {
      target.textContent = (target.textContent || '') + text;
      target.dispatchEvent(new Event('input', { bubbles: true }));
      return `type: text+input (${text.length} chars appended)`;
    }
    return 'type: target is not editable';
  }

  if (action === 'key') {
    if (typeof target.focus === 'function') target.focus();
    const init: KeyboardEventInit = {
      key: detail.key || '',
      bubbles: true,
      cancelable: true,
      ...modifierFlags(detail),
    };
    target.dispatchEvent(new KeyboardEvent('keydown', init));
    target.dispatchEvent(new KeyboardEvent('keyup', init));
    return `key: keydown,keyup (${detail.key})`;
  }

  if (action === 'scroll') {
    const amount = detail.scrollAmount ?? 0;
    const container =
      findScrollableContainer(el) ??
      (document.scrollingElement as HTMLElement | null);
    if (container) {
      const delta = amount * 120;
      if ((detail.axis || 'vertical').toLowerCase() === 'horizontal') {
        container.scrollLeft += delta;
      } else {
        container.scrollTop += delta;
      }
      container.dispatchEvent(new Event('scroll', { bubbles: true }));
      return `scroll: ${delta >= 0 ? '+' : ''}${delta}px ${detail.axis || 'vertical'}`;
    }
    return 'scroll: no scrollable container found';
  }

  return `unsupported action '${action}'`;
}

async function handleInput(detail: InputEvent): Promise<void> {
  const el = resolveTarget(detail);
  if (!el) {
    await invoke('e2e_ui_report', {
      requestId: detail.requestId,
      payload: { found: false },
    }).catch(() => {});
    return;
  }
  if (typeof el.scrollIntoView === 'function') {
    el.scrollIntoView({ block: 'nearest', inline: 'nearest' });
  }
  const description = dispatchDomInput(el, detail);
  await invoke('e2e_ui_report', {
    requestId: detail.requestId,
    payload: {
      found: true,
      dispatched: !description.startsWith('unsupported action'),
      action: detail.action,
      description,
      rect: rectOf(el),
    },
  }).catch(() => {});
}

interface NavigateEvent {
  path?: string | null;
  panel?: string | null;
}

function dispatchPanelEvent(panel: string): void {
  window.__LIBRAGENT_E2E_PENDING_PANEL = panel;
  window.dispatchEvent(new CustomEvent(E2E_PANEL_EVENT, { detail: { panel } }));
  setTimeout(() => {
    window.dispatchEvent(
      new CustomEvent(E2E_PANEL_EVENT, { detail: { panel } }),
    );
  }, 150);
}

/**
 * Mount inside the router. Registers the control-plane listeners only after
 * the backend confirms the control plane is enabled; renders nothing.
 */
export function E2eUiBridge(): null {
  const navigate = useNavigate();
  const navigateRef = useRef(navigate);
  navigateRef.current = navigate;

  useEffect(() => {
    let cancelled = false;
    const unlisten: UnlistenFn[] = [];

    invoke<boolean>('e2e_ui_enabled')
      .then(async (enabled) => {
        if (cancelled || !enabled) return;
        bridgeActive = true;
        // Sync the backend counter with the current number of mounted dialogs.
        await invoke('e2e_ui_report_dialog', {
          open: false,
          reset: true,
        }).catch(() => {});
        for (let i = 0; i < mountedDialogCount; i += 1) {
          await invoke('e2e_ui_report_dialog', { open: true }).catch(() => {});
        }

        unlisten.push(
          await listen<NavigateEvent>('e2e-navigate', (event) => {
            if (event.payload.path) {
              navigateRef.current(event.payload.path);
            }
            if (event.payload.panel) {
              dispatchPanelEvent(event.payload.panel);
            }
          }),
        );
        unlisten.push(
          await listen<InspectEvent>('e2e-inspect', (event) => {
            void handleInspect(event.payload);
          }),
        );
        unlisten.push(
          await listen<InputEvent>('e2e-input', (event) => {
            void handleInput(event.payload);
          }),
        );
      })
      .catch(() => {
        /* command absent (release build): stay dormant */
      });

    return () => {
      cancelled = true;
      bridgeActive = false;
      unlisten.forEach((u) => u());
    };
  }, []);

  return null;
}
