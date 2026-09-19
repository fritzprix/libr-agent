/**
 * UI tool-injection policy (negative list).
 *
 * SSOT for starting an LLM turn is `submit()` (chat input path).
 * UI-recorded tool results are always persisted with `appendToolMessages`
 * (never `injectMessages`), then optionally followed by `submit()`.
 *
 * Default: history-then-submit (record tool pair, then kick off LLM via submit).
 * Negative list: history-only — transcript side-effects that must NOT start a turn.
 *
 * Add a tool here when the UI already performed the work and the model should
 * not be asked to react (e.g. workspace file import).
 */
export const UI_TOOL_HISTORY_ONLY = [
  'workspace__importFiles',
] as const;

export type UiToolHistoryOnlyName = (typeof UI_TOOL_HISTORY_ONLY)[number];

const UI_TOOL_HISTORY_ONLY_SET: ReadonlySet<string> = new Set(
  UI_TOOL_HISTORY_ONLY,
);

export type UiToolInjectionMode = 'history-only' | 'history-then-submit';

/** Resolve whether a UI-recorded tool should kick off an LLM turn via submit. */
export function getUiToolInjectionMode(toolName: string): UiToolInjectionMode {
  return UI_TOOL_HISTORY_ONLY_SET.has(toolName)
    ? 'history-only'
    : 'history-then-submit';
}

export function isUiToolHistoryOnly(toolName: string): boolean {
  return getUiToolInjectionMode(toolName) === 'history-only';
}
