import { describe, expect, it } from 'vitest';
import { act, renderHook } from '@testing-library/react';
import type { ReactNode } from 'react';
import {
  AgentFilePreviewProvider,
  useAgentFilePreview,
  useOptionalAgentFilePreview,
} from '../AgentFilePreviewContext';

function wrapper({ children }: { children: ReactNode }) {
  return <AgentFilePreviewProvider>{children}</AgentFilePreviewProvider>;
}

describe('AgentFilePreviewContext', () => {
  it('opens and closes a preview target', () => {
    const { result } = renderHook(() => useAgentFilePreview(), { wrapper });

    expect(result.current.previewFile).toBeNull();

    act(() => {
      result.current.openFilePreview({
        path: 'src/notes.md',
        name: 'notes.md',
        size: 120,
        sessionId: 'session-1',
      });
    });

    expect(result.current.previewFile).toEqual({
      path: 'src/notes.md',
      name: 'notes.md',
      size: 120,
      sessionId: 'session-1',
    });

    act(() => {
      result.current.closeFilePreview();
    });

    expect(result.current.previewFile).toBeNull();
  });

  it('replaces the current preview when opening another file', () => {
    const { result } = renderHook(() => useAgentFilePreview(), { wrapper });

    act(() => {
      result.current.openFilePreview({
        path: 'a.md',
        name: 'a.md',
      });
    });
    act(() => {
      result.current.openFilePreview({
        path: 'b.ts',
        name: 'b.ts',
        sessionId: 'session-2',
      });
    });

    expect(result.current.previewFile?.path).toBe('b.ts');
    expect(result.current.previewFile?.sessionId).toBe('session-2');
  });

  it('throws when used outside the provider', () => {
    expect(() => renderHook(() => useAgentFilePreview())).toThrow(
      /AgentFilePreviewProvider/,
    );
  });

  it('optional hook returns undefined outside the provider', () => {
    const { result } = renderHook(() => useOptionalAgentFilePreview());
    expect(result.current).toBeUndefined();
  });
});
