import { renderHook } from '@testing-library/react';
import { useRustBackend } from '../use-rust-backend';
import { describe, it, expect } from 'vitest';

describe('useRustBackend', () => {
  it('should return a stable object reference across renders', () => {
    const { result, rerender } = renderHook(() => useRustBackend());
    const firstResult = result.current;

    rerender();
    const secondResult = result.current;

    expect(secondResult).toBe(firstResult);
  });
});
