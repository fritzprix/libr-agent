import { describe, expect, it } from 'vitest';

import {
  getUiToolInjectionMode,
  isUiToolHistoryOnly,
  UI_TOOL_HISTORY_ONLY,
} from '../uiToolInjectionPolicy';

describe('uiToolInjectionPolicy', () => {
  it('lists importFiles as history-only (neg-list)', () => {
    expect(UI_TOOL_HISTORY_ONLY).toContain('workspace__importFiles');
    expect(isUiToolHistoryOnly('workspace__importFiles')).toBe(true);
    expect(getUiToolInjectionMode('workspace__importFiles')).toBe(
      'history-only',
    );
  });

  it('defaults unknown / playbook tools to history-then-submit', () => {
    expect(getUiToolInjectionMode('playbook__selectPlaybook')).toBe(
      'history-then-submit',
    );
    expect(isUiToolHistoryOnly('playbook__selectPlaybook')).toBe(false);
    expect(getUiToolInjectionMode('some__otherTool')).toBe(
      'history-then-submit',
    );
  });
});
