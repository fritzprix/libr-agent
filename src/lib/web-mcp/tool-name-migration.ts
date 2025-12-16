/**
 * Tool Name Migration Utility
 *
 * Handles backward compatibility for tool names during refactoring.
 * Maps old tool names to new standardized names.
 */

// Map of old names to new names
const MIGRATION_MAP: Record<string, string> = {
  // Planning Server Migrations (snake_case -> camelCase)
  create_goal: 'createGoal',
  update_goal: 'updateGoal',
  clear_goal: 'clearGoal',
  add_todo: 'addTodo',
  update_todo: 'updateTodo',
  mark_todo: 'markTodo',
  clear_todos: 'clearTodos',
  clear_session: 'clearSession',
  add_scratchpad: 'addScratchpad',
  clear_scratchpad: 'clearScratchpad',
  get_current_state: 'getCurrentState',
  pause_and_think: 'pauseAndThink',
  critique_and_reflection: 'critiqueAndReflection',
};

export function resolveToolName(name: string): {
  resolvedName: string;
  isDeprecated: boolean;
} {
  const newName = MIGRATION_MAP[name];
  if (newName) {
    return { resolvedName: newName, isDeprecated: true };
  }
  return { resolvedName: name, isDeprecated: false };
}

export function logDeprecationWarning(oldName: string, newName: string): void {
  console.warn(
    `[Deprecation] Tool "${oldName}" is deprecated. Please use "${newName}" instead.`,
  );
}
