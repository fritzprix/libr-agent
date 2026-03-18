import { describe, it, expect } from 'vitest';
import { calculatePlanningMetadata } from '../planning';

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
});
