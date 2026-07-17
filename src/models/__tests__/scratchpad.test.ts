import { describe, it, expect } from 'vitest';
import { parseScratchpadState } from '../scratchpad';

describe('Models Scratchpad', () => {
  describe('parseScratchpadState', () => {
    it('parses scratchpad items from structured context', () => {
      const state = parseScratchpadState({
        count: 2,
        items: [
          { id: 10, title: 'Note A', content: 'Alpha' },
          { id: 11, title: null, content: 'Beta' },
        ],
      });

      expect(state).toEqual({
        count: 2,
        items: [
          { id: 10, title: 'Note A', content: 'Alpha' },
          { id: 11, title: null, content: 'Beta' },
        ],
      });
    });

    it('falls back to item length when count is missing', () => {
      const state = parseScratchpadState({
        items: [{ id: 1, title: 'Only note', content: 'Content' }],
      });

      expect(state).toEqual({
        count: 1,
        items: [{ id: 1, title: 'Only note', content: 'Content' }],
      });
    });
  });
});
