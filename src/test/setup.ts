import { expect, afterEach, vi } from 'vitest';
import { cleanup } from '@testing-library/react';
import * as matchers from '@testing-library/jest-dom/matchers';

// Mock Tauri APIs for testing environment
Object.defineProperty(window, '__TAURI_INTERNALS__', {
  value: {
    invoke: vi.fn().mockResolvedValue(undefined),
  },
});

// Mock CSS.escape if not available
if (typeof CSS === 'undefined' || !CSS.escape) {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (global as any).CSS = {
    escape: (str: string) =>
      str.replace(/([!"#$%&'()*+,\-./:;<=>?@[\\\]^`{|}~])/g, '\\$1'),
  };
}

// Extend expect with jest-dom matchers
expect.extend(matchers);

// Clean up after each test
afterEach(() => {
  cleanup();
});
