import { act, renderHook } from '@testing-library/react';
import { describe, it, expect } from 'vitest';
import { useInputToken } from '../useInputToken';

describe('useInputToken', () => {
  it('should identify a command when typing starts with /', () => {
    const { result } = renderHook(() => useInputToken([], []));

    act(() => {
      result.current.onInputChange('/clear', 6);
    });

    expect(result.current.stage).toEqual({
      kind: 'typing-command',
      query: 'clear',
      anchorIndex: 0,
    });
    expect(result.current.commandResults).toHaveLength(1);
    expect(result.current.commandResults[0].id).toBe('/clear');
  });

  it('should identify a command when typing / after a space', () => {
    const { result } = renderHook(() => useInputToken([], []));

    act(() => {
      result.current.onInputChange('hello /clear', 12);
    });

    expect(result.current.stage).toEqual({
      kind: 'typing-command',
      query: 'clear',
      anchorIndex: 6,
    });
    expect(result.current.commandResults).toHaveLength(1);
  });

  it('should not identify a command when / is preceded by a non-whitespace character', () => {
    const { result } = renderHook(() => useInputToken([], []));

    act(() => {
      result.current.onInputChange('hello/clear', 11);
    });

    expect(result.current.stage).toEqual({ kind: 'idle' });
    expect(result.current.commandResults).toHaveLength(0);
  });

  it('should not identify a command for path slashes', () => {
    const { result } = renderHook(() => useInputToken([], []));

    act(() => {
      result.current.onInputChange('src/features/agent', 18);
    });

    expect(result.current.stage).toEqual({ kind: 'idle' });
    expect(result.current.commandResults).toHaveLength(0);
  });
});
