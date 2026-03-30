import { describe, it, expect } from 'vitest';
import {
  calculatePlanningMetadata,
  parsePlanningState,
  parseScratchpadState,
} from '../planning';

describe('Models Planning', () => {
  describe('calculatePlanningMetadata', () => {
    it('returns zeroes when state is undefined', () => {
      const metadata = calculatePlanningMetadata(undefined);
      expect(metadata).toEqual({
        totalTodos: 0,
        completedTodos: 0,
        activeTodos: 0,
      });
    });

    it('returns zeroes when state has no todos', () => {
      const state = {
        goal: 'test',
        todos: [],
      };
      const metadata = calculatePlanningMetadata(state);
      expect(metadata).toEqual({
        totalTodos: 0,
        completedTodos: 0,
        activeTodos: 0,
      });
    });

    it('calculates correctly for mixed completed and active todos', () => {
      const state = {
        goal: 'test goal',
        todos: [
          { id: 1, title: 'todo 1', checked: true },
          { id: 2, title: 'todo 2', checked: false },
          { id: 3, title: 'todo 3', checked: true },
          { id: 4, title: 'todo 4', checked: false },
        ],
      };
      const metadata = calculatePlanningMetadata(state);
      expect(metadata).toEqual({
        totalTodos: 4,
        completedTodos: 2,
        activeTodos: 2,
      });
    });

    it('calculates correctly when all todos are completed', () => {
      const state = {
        goal: 'test goal',
        todos: [
          { id: 1, title: 'todo 1', checked: true },
          { id: 2, title: 'todo 2', checked: true },
        ],
      };
      const metadata = calculatePlanningMetadata(state);
      expect(metadata).toEqual({
        totalTodos: 2,
        completedTodos: 2,
        activeTodos: 0,
      });
    });

    it('calculates correctly when all todos are active', () => {
      const state = {
        goal: 'test goal',
        todos: [
          { id: 1, title: 'todo 1', checked: false },
          { id: 2, title: 'todo 2', checked: false },
        ],
      };
      const metadata = calculatePlanningMetadata(state);
      expect(metadata).toEqual({
        totalTodos: 2,
        completedTodos: 0,
        activeTodos: 2,
      });
    });
  });

  describe('parsePlanningState', () => {
    it('parses valid planning state from structured context', () => {
      const state = parsePlanningState({
        goal: 'Ship planning panel',
        todos: [
          {
            id: 1,
            title: 'Show scratchpad',
            checked: false,
            priority: 'high',
            description: 'Render scratchpad notes',
          },
        ],
        lastUpdated: '2026-03-30T00:00:00Z',
      });

      expect(state).toEqual({
        goal: 'Ship planning panel',
        todos: [
          {
            id: 1,
            title: 'Show scratchpad',
            checked: false,
            priority: 'high',
            description: 'Render scratchpad notes',
            summary: undefined,
            parentId: undefined,
            subtasks: undefined,
          },
        ],
        lastUpdated: '2026-03-30T00:00:00Z',
      });
    });

    it('returns an empty normalized state for invalid todo payloads', () => {
      const state = parsePlanningState({
        goal: 'Keep going',
        todos: [{ id: 'bad', title: 123, checked: 'nope' }],
      });

      expect(state).toEqual({
        goal: 'Keep going',
        todos: [],
        lastUpdated: undefined,
      });
    });
  });

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
