/**
 * App-control event payloads emitted from Rust (`libragent:app-control`)
 * and consumed by the React bridge for remote UI automation.
 */

export type AppControlNavigate = {
  op: 'navigate';
  path: string;
};

export type AppControlHighlight = {
  op: 'highlight';
  target: 'preset';
  name: string;
  ms?: number;
};

export type AppControlInstallPreset = {
  op: 'install_preset';
  name: string;
};

export type AppControlFocusSession = {
  op: 'focus_session';
  sessionId: string;
};

export type AppControlPayload =
  | AppControlNavigate
  | AppControlHighlight
  | AppControlInstallPreset
  | AppControlFocusSession;

export function isAppControlPayload(
  value: unknown,
): value is AppControlPayload {
  if (!value || typeof value !== 'object') {
    return false;
  }
  const op = (value as { op?: unknown }).op;
  if (typeof op !== 'string') {
    return false;
  }
  switch (op) {
    case 'navigate':
      return typeof (value as AppControlNavigate).path === 'string';
    case 'highlight':
      return (
        (value as AppControlHighlight).target === 'preset' &&
        typeof (value as AppControlHighlight).name === 'string'
      );
    case 'install_preset':
      return typeof (value as AppControlInstallPreset).name === 'string';
    case 'focus_session':
      return typeof (value as AppControlFocusSession).sessionId === 'string';
    default:
      return false;
  }
}
