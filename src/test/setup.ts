import { expect, afterEach, vi } from 'vitest';
import { cleanup } from '@testing-library/react';
import * as matchers from '@testing-library/jest-dom/matchers';
import 'fake-indexeddb/auto';

// Mock Tauri APIs for testing environment
Object.defineProperty(window, '__TAURI_INTERNALS__', {
  value: {
    invoke: vi.fn().mockResolvedValue(undefined),
    // Mock transformCallback for event listeners
    transformCallback: vi.fn((callback?: unknown) => callback),
  },
});

// Mock Tauri event plugin internals
Object.defineProperty(window, '__TAURI_EVENT_PLUGIN_INTERNALS__', {
  value: {
    unregisterListener: vi.fn(),
  },
});

// Mock CSS.escape if not available
if (typeof CSS === 'undefined' || !CSS.escape) {
  (global as unknown as { CSS: { escape: (str: string) => string } }).CSS = {
    escape: (str: string) =>
      str.replace(/([!"#$%&'()*+,\-./:;<=>?@[\\\]^`{|}~])/g, '\\$1'),
  };
}

// Extend expect with jest-dom matchers
expect.extend(matchers);

// Clean up after each test
afterEach(async () => {
  cleanup();
  // Reset IndexedDB to ensure isolation between tests
  // Delete all databases to ensure complete isolation
  const databases = await indexedDB.databases();
  for (const db of databases) {
    if (db.name) {
      indexedDB.deleteDatabase(db.name);
    }
  }
});
